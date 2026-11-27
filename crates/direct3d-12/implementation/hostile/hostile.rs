//! Written by tools/api-crate-generator from the Win32 metadata, version 63.0.31, and never by hand.
//!
//! The hostile implementation (src/glue_hostile.rs) of direct3d-12's traits, for tests only: every call is logged by its place, with the values
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
#[path = "../direct/data.rs"]
mod data_direct3d_12;
#[path = "../../../directx-graphics-infrastructure/implementation/direct/data.rs"]
mod data_directx_graphics_infrastructure;

use wcontainers_component_object_model as component_object_model;
use wcontainers_direct3d_12 as direct3d_12;
use wcontainers_direct3d_12_direct as direct3d_12_direct;
use wcontainers_direct3d_common as direct3d_common;
use wcontainers_direct3d_common_direct as direct3d_common_direct;
use wcontainers_direct3d_common_hostile as direct3d_common_hostile;

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

/// A recorded object of the kind `CommandAllocator`.
pub struct HostileCommandAllocator<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> HostileCommandAllocator<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> HostileCommandAllocator<A> {
        HostileCommandAllocator {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for HostileCommandAllocator<A> {
    fn clone(&self) -> HostileCommandAllocator<A> {
        never_asked(CLONE_NAME)
    }
}

impl<A> PartialEq for HostileCommandAllocator<A> {
    fn eq(&self, other: &HostileCommandAllocator<A>) -> bool {
        never_asked(PARTIAL_EQ_NAME)
    }
}

impl<A> Eq for HostileCommandAllocator<A> {}

impl<A> Hash for HostileCommandAllocator<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        never_asked(HASH_NAME)
    }
}

impl<A> core::fmt::Debug for HostileCommandAllocator<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("HostileCommandAllocator")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `CommandQueue`.
pub struct HostileCommandQueue<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> HostileCommandQueue<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> HostileCommandQueue<A> {
        HostileCommandQueue {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for HostileCommandQueue<A> {
    fn clone(&self) -> HostileCommandQueue<A> {
        never_asked(CLONE_NAME)
    }
}

impl<A> PartialEq for HostileCommandQueue<A> {
    fn eq(&self, other: &HostileCommandQueue<A>) -> bool {
        never_asked(PARTIAL_EQ_NAME)
    }
}

impl<A> Eq for HostileCommandQueue<A> {}

impl<A> Hash for HostileCommandQueue<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        never_asked(HASH_NAME)
    }
}

impl<A> core::fmt::Debug for HostileCommandQueue<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("HostileCommandQueue")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `DescriptorHeap`.
pub struct HostileDescriptorHeap<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> HostileDescriptorHeap<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> HostileDescriptorHeap<A> {
        HostileDescriptorHeap {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for HostileDescriptorHeap<A> {
    fn clone(&self) -> HostileDescriptorHeap<A> {
        never_asked(CLONE_NAME)
    }
}

impl<A> PartialEq for HostileDescriptorHeap<A> {
    fn eq(&self, other: &HostileDescriptorHeap<A>) -> bool {
        never_asked(PARTIAL_EQ_NAME)
    }
}

impl<A> Eq for HostileDescriptorHeap<A> {}

impl<A> Hash for HostileDescriptorHeap<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        never_asked(HASH_NAME)
    }
}

impl<A> core::fmt::Debug for HostileDescriptorHeap<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("HostileDescriptorHeap")
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

/// A recorded object of the kind `Fence`.
pub struct HostileFence<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> HostileFence<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> HostileFence<A> {
        HostileFence {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for HostileFence<A> {
    fn clone(&self) -> HostileFence<A> {
        never_asked(CLONE_NAME)
    }
}

impl<A> PartialEq for HostileFence<A> {
    fn eq(&self, other: &HostileFence<A>) -> bool {
        never_asked(PARTIAL_EQ_NAME)
    }
}

impl<A> Eq for HostileFence<A> {}

impl<A> Hash for HostileFence<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        never_asked(HASH_NAME)
    }
}

impl<A> core::fmt::Debug for HostileFence<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("HostileFence")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `GraphicsCommandList`.
pub struct HostileGraphicsCommandList<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> HostileGraphicsCommandList<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> HostileGraphicsCommandList<A> {
        HostileGraphicsCommandList {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for HostileGraphicsCommandList<A> {
    fn clone(&self) -> HostileGraphicsCommandList<A> {
        never_asked(CLONE_NAME)
    }
}

impl<A> PartialEq for HostileGraphicsCommandList<A> {
    fn eq(&self, other: &HostileGraphicsCommandList<A>) -> bool {
        never_asked(PARTIAL_EQ_NAME)
    }
}

impl<A> Eq for HostileGraphicsCommandList<A> {}

impl<A> Hash for HostileGraphicsCommandList<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        never_asked(HASH_NAME)
    }
}

impl<A> core::fmt::Debug for HostileGraphicsCommandList<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("HostileGraphicsCommandList")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `PipelineState`.
pub struct HostilePipelineState<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> HostilePipelineState<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> HostilePipelineState<A> {
        HostilePipelineState {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for HostilePipelineState<A> {
    fn clone(&self) -> HostilePipelineState<A> {
        never_asked(CLONE_NAME)
    }
}

impl<A> PartialEq for HostilePipelineState<A> {
    fn eq(&self, other: &HostilePipelineState<A>) -> bool {
        never_asked(PARTIAL_EQ_NAME)
    }
}

impl<A> Eq for HostilePipelineState<A> {}

impl<A> Hash for HostilePipelineState<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        never_asked(HASH_NAME)
    }
}

impl<A> core::fmt::Debug for HostilePipelineState<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("HostilePipelineState")
            .field(&self.live.0)
            .finish()
    }
}

/// A recorded object of the kind `QueryHeap`.
pub struct HostileQueryHeap<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> HostileQueryHeap<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> HostileQueryHeap<A> {
        HostileQueryHeap {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for HostileQueryHeap<A> {
    fn clone(&self) -> HostileQueryHeap<A> {
        never_asked(CLONE_NAME)
    }
}

impl<A> PartialEq for HostileQueryHeap<A> {
    fn eq(&self, other: &HostileQueryHeap<A>) -> bool {
        never_asked(PARTIAL_EQ_NAME)
    }
}

impl<A> Eq for HostileQueryHeap<A> {}

impl<A> Hash for HostileQueryHeap<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        never_asked(HASH_NAME)
    }
}

impl<A> core::fmt::Debug for HostileQueryHeap<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("HostileQueryHeap")
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

/// A recorded object of the kind `RootSignature`.
pub struct HostileRootSignature<A> {
    live: Arc<Live>,
    api: PhantomData<fn() -> A>,
}

impl<A> HostileRootSignature<A> {
    /// A new object, of an identity of its own.
    pub fn new() -> HostileRootSignature<A> {
        HostileRootSignature {
            live: Live::new(),
            api: PhantomData,
        }
    }

    /// The object's identity.
    pub fn identity(&self) -> u64 {
        self.live.0
    }
}

impl<A> Clone for HostileRootSignature<A> {
    fn clone(&self) -> HostileRootSignature<A> {
        never_asked(CLONE_NAME)
    }
}

impl<A> PartialEq for HostileRootSignature<A> {
    fn eq(&self, other: &HostileRootSignature<A>) -> bool {
        never_asked(PARTIAL_EQ_NAME)
    }
}

impl<A> Eq for HostileRootSignature<A> {}

impl<A> Hash for HostileRootSignature<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        never_asked(HASH_NAME)
    }
}

impl<A> core::fmt::Debug for HostileRootSignature<A> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("HostileRootSignature")
            .field(&self.live.0)
            .finish()
    }
}

/// An API value whose objects are this hostile implementation's.
pub trait Hostile:
    direct3d_12::Direct3D12<
        CommandAllocator = HostileCommandAllocator<Self>,
        CommandQueue = HostileCommandQueue<Self>,
        DescriptorHeap = HostileDescriptorHeap<Self>,
        Device = HostileDevice<Self>,
        Fence = HostileFence<Self>,
        GraphicsCommandList = HostileGraphicsCommandList<Self>,
        PipelineState = HostilePipelineState<Self>,
        QueryHeap = HostileQueryHeap<Self>,
        Resource = HostileResource<Self>,
        RootSignature = HostileRootSignature<Self>,
    > + direct3d_common_hostile::Hostile
{
}

impl<A: Hostile> direct3d_12::CommandAllocator for HostileCommandAllocator<A> {
    fn reset(&self) -> Result<(), component_object_model::structures::HResult> {
        record("ID3D12CommandAllocator::Reset()".to_string());
        Ok(())
    }
}

impl<A: Hostile> direct3d_12::Pageable for HostileCommandAllocator<A> {}

impl<A: Hostile> direct3d_12::DeviceChild for HostileCommandAllocator<A> {}

impl<A: Hostile> direct3d_12::Object for HostileCommandAllocator<A> {
    type Api = A;
}

impl<A: Hostile> direct3d_12::CommandQueue for HostileCommandQueue<A> {
    fn execute_command_lists(
        &self,
        command_lists: &[Option<direct3d_12::CommandListRef<'_, Self::Api>>],
    ) {
        let _ = &command_lists;
        record("ID3D12CommandQueue::ExecuteCommandLists()".to_string());
    }
    fn signal(
        &self,
        fence: &<Self::Api as direct3d_12::Direct3D12>::Fence,
        value: u64,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &fence;
        let _ = &value;
        record(format!("ID3D12CommandQueue::Signal(value={:?})", value));
        Ok(())
    }
}

impl<A: Hostile> direct3d_12::Pageable for HostileCommandQueue<A> {}

impl<A: Hostile> direct3d_12::DeviceChild for HostileCommandQueue<A> {}

impl<A: Hostile> direct3d_12::Object for HostileCommandQueue<A> {
    type Api = A;
}

impl<A: Hostile> direct3d_12::DescriptorHeap for HostileDescriptorHeap<A> {
    fn get_desc(&self) -> direct3d_12::structures::DescriptorHeapDesc {
        record("ID3D12DescriptorHeap::GetDesc()".to_string());
        data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_descriptor_heap_desc(
            &[u8::MIN; 16],
        )
    }
    fn get_cpu_descriptor_handle_for_heap_start(
        &self,
    ) -> direct3d_12::structures::CpuDescriptorHandle {
        record("ID3D12DescriptorHeap::GetCPUDescriptorHandleForHeapStart()".to_string());
        data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_cpu_descriptor_handle(
            &[u8::MIN; 8],
        )
    }
    fn get_gpu_descriptor_handle_for_heap_start(
        &self,
    ) -> direct3d_12::structures::GpuDescriptorHandle {
        record("ID3D12DescriptorHeap::GetGPUDescriptorHandleForHeapStart()".to_string());
        data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_gpu_descriptor_handle(
            &[u8::MIN; 8],
        )
    }
}

impl<A: Hostile> direct3d_12::Pageable for HostileDescriptorHeap<A> {}

impl<A: Hostile> direct3d_12::DeviceChild for HostileDescriptorHeap<A> {}

impl<A: Hostile> direct3d_12::Object for HostileDescriptorHeap<A> {
    type Api = A;
}

impl<A: Hostile> direct3d_12::Device for HostileDevice<A> {
    fn create_command_queue(
        &self,
        desc: &direct3d_12::structures::CommandQueueDesc,
        riid: &component_object_model::structures::Guid,
    ) -> Result<direct3d_12::UnknownObject<Self::Api>, component_object_model::structures::HResult>
    {
        let _ = &desc;
        let _ = &riid;
        record("ID3D12Device::CreateCommandQueue()".to_string());
        Ok(
            if [
                direct3d_12::constants::IID_ID3D12_COMMAND_ALLOCATOR,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::CommandAllocator(HostileCommandAllocator::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_COMMAND_QUEUE,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::CommandQueue(HostileCommandQueue::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DESCRIPTOR_HEAP,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::DescriptorHeap(HostileDescriptorHeap::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE,
                direct3d_12::constants::IID_ID3D12_OBJECT,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::Device(HostileDevice::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_FENCE,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::Fence(HostileFence::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_COMMAND_LIST,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_GRAPHICS_COMMAND_LIST,
                direct3d_12::constants::IID_ID3D12_OBJECT,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::GraphicsCommandList(HostileGraphicsCommandList::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
                direct3d_12::constants::IID_ID3D12_PIPELINE_STATE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::PipelineState(HostilePipelineState::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
                direct3d_12::constants::IID_ID3D12_QUERY_HEAP,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::QueryHeap(HostileQueryHeap::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
                direct3d_12::constants::IID_ID3D12_RESOURCE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::Resource(HostileResource::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_ROOT_SIGNATURE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::RootSignature(HostileRootSignature::new())
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
    fn create_command_allocator(
        &self,
        type_: direct3d_12::enumerations::CommandListType,
        riid: &component_object_model::structures::Guid,
    ) -> Result<direct3d_12::UnknownObject<Self::Api>, component_object_model::structures::HResult>
    {
        let _ = &type_;
        let _ = &riid;
        record(format!(
            "ID3D12Device::CreateCommandAllocator(type_={:?})",
            type_
        ));
        Ok(
            if [
                direct3d_12::constants::IID_ID3D12_COMMAND_ALLOCATOR,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::CommandAllocator(HostileCommandAllocator::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_COMMAND_QUEUE,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::CommandQueue(HostileCommandQueue::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DESCRIPTOR_HEAP,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::DescriptorHeap(HostileDescriptorHeap::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE,
                direct3d_12::constants::IID_ID3D12_OBJECT,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::Device(HostileDevice::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_FENCE,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::Fence(HostileFence::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_COMMAND_LIST,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_GRAPHICS_COMMAND_LIST,
                direct3d_12::constants::IID_ID3D12_OBJECT,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::GraphicsCommandList(HostileGraphicsCommandList::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
                direct3d_12::constants::IID_ID3D12_PIPELINE_STATE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::PipelineState(HostilePipelineState::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
                direct3d_12::constants::IID_ID3D12_QUERY_HEAP,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::QueryHeap(HostileQueryHeap::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
                direct3d_12::constants::IID_ID3D12_RESOURCE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::Resource(HostileResource::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_ROOT_SIGNATURE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::RootSignature(HostileRootSignature::new())
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
    fn create_graphics_pipeline_state(
        &self,
        desc: &direct3d_12::structures::GraphicsPipelineStateDesc<Self::Api>,
        riid: &component_object_model::structures::Guid,
    ) -> Result<direct3d_12::UnknownObject<Self::Api>, component_object_model::structures::HResult>
    {
        let _ = &desc;
        let _ = &riid;
        record("ID3D12Device::CreateGraphicsPipelineState()".to_string());
        Err(E_NOTIMPL)
    }
    fn create_compute_pipeline_state(
        &self,
        desc: &direct3d_12::structures::ComputePipelineStateDesc<Self::Api>,
        riid: &component_object_model::structures::Guid,
    ) -> Result<direct3d_12::UnknownObject<Self::Api>, component_object_model::structures::HResult>
    {
        let _ = &desc;
        let _ = &riid;
        record("ID3D12Device::CreateComputePipelineState()".to_string());
        Err(E_NOTIMPL)
    }
    fn create_command_list(
        &self,
        node_mask: u32,
        type_: direct3d_12::enumerations::CommandListType,
        command_allocator: &<Self::Api as direct3d_12::Direct3D12>::CommandAllocator,
        initial_state: Option<&<Self::Api as direct3d_12::Direct3D12>::PipelineState>,
        riid: &component_object_model::structures::Guid,
    ) -> Result<direct3d_12::UnknownObject<Self::Api>, component_object_model::structures::HResult>
    {
        let _ = &node_mask;
        let _ = &type_;
        let _ = &command_allocator;
        let _ = &initial_state;
        let _ = &riid;
        record(format!(
            "ID3D12Device::CreateCommandList(node_mask={:?}, type_={:?})",
            node_mask, type_
        ));
        Ok(
            if [
                direct3d_12::constants::IID_ID3D12_COMMAND_ALLOCATOR,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::CommandAllocator(HostileCommandAllocator::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_COMMAND_QUEUE,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::CommandQueue(HostileCommandQueue::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DESCRIPTOR_HEAP,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::DescriptorHeap(HostileDescriptorHeap::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE,
                direct3d_12::constants::IID_ID3D12_OBJECT,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::Device(HostileDevice::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_FENCE,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::Fence(HostileFence::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_COMMAND_LIST,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_GRAPHICS_COMMAND_LIST,
                direct3d_12::constants::IID_ID3D12_OBJECT,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::GraphicsCommandList(HostileGraphicsCommandList::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
                direct3d_12::constants::IID_ID3D12_PIPELINE_STATE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::PipelineState(HostilePipelineState::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
                direct3d_12::constants::IID_ID3D12_QUERY_HEAP,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::QueryHeap(HostileQueryHeap::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
                direct3d_12::constants::IID_ID3D12_RESOURCE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::Resource(HostileResource::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_ROOT_SIGNATURE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::RootSignature(HostileRootSignature::new())
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
    fn create_descriptor_heap(
        &self,
        descriptor_heap_desc: &direct3d_12::structures::DescriptorHeapDesc,
        riid: &component_object_model::structures::Guid,
    ) -> Result<direct3d_12::UnknownObject<Self::Api>, component_object_model::structures::HResult>
    {
        let _ = &descriptor_heap_desc;
        let _ = &riid;
        record("ID3D12Device::CreateDescriptorHeap()".to_string());
        Ok(
            if [
                direct3d_12::constants::IID_ID3D12_COMMAND_ALLOCATOR,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::CommandAllocator(HostileCommandAllocator::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_COMMAND_QUEUE,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::CommandQueue(HostileCommandQueue::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DESCRIPTOR_HEAP,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::DescriptorHeap(HostileDescriptorHeap::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE,
                direct3d_12::constants::IID_ID3D12_OBJECT,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::Device(HostileDevice::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_FENCE,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::Fence(HostileFence::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_COMMAND_LIST,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_GRAPHICS_COMMAND_LIST,
                direct3d_12::constants::IID_ID3D12_OBJECT,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::GraphicsCommandList(HostileGraphicsCommandList::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
                direct3d_12::constants::IID_ID3D12_PIPELINE_STATE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::PipelineState(HostilePipelineState::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
                direct3d_12::constants::IID_ID3D12_QUERY_HEAP,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::QueryHeap(HostileQueryHeap::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
                direct3d_12::constants::IID_ID3D12_RESOURCE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::Resource(HostileResource::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_ROOT_SIGNATURE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::RootSignature(HostileRootSignature::new())
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
    fn get_descriptor_handle_increment_size(
        &self,
        descriptor_heap_type: direct3d_12::enumerations::DescriptorHeapType,
    ) -> u32 {
        let _ = &descriptor_heap_type;
        record(format!(
            "ID3D12Device::GetDescriptorHandleIncrementSize(descriptor_heap_type={:?})",
            descriptor_heap_type
        ));
        Default::default()
    }
    fn create_root_signature(
        &self,
        node_mask: u32,
        blob_with_root_signature: &[u8],
        riid: &component_object_model::structures::Guid,
    ) -> Result<direct3d_12::UnknownObject<Self::Api>, component_object_model::structures::HResult>
    {
        let _ = &node_mask;
        let _ = &blob_with_root_signature;
        let _ = &riid;
        record(format!(
            "ID3D12Device::CreateRootSignature(node_mask={:?})",
            node_mask
        ));
        Ok(
            if [
                direct3d_12::constants::IID_ID3D12_COMMAND_ALLOCATOR,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::CommandAllocator(HostileCommandAllocator::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_COMMAND_QUEUE,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::CommandQueue(HostileCommandQueue::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DESCRIPTOR_HEAP,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::DescriptorHeap(HostileDescriptorHeap::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE,
                direct3d_12::constants::IID_ID3D12_OBJECT,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::Device(HostileDevice::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_FENCE,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::Fence(HostileFence::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_COMMAND_LIST,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_GRAPHICS_COMMAND_LIST,
                direct3d_12::constants::IID_ID3D12_OBJECT,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::GraphicsCommandList(HostileGraphicsCommandList::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
                direct3d_12::constants::IID_ID3D12_PIPELINE_STATE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::PipelineState(HostilePipelineState::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
                direct3d_12::constants::IID_ID3D12_QUERY_HEAP,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::QueryHeap(HostileQueryHeap::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
                direct3d_12::constants::IID_ID3D12_RESOURCE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::Resource(HostileResource::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_ROOT_SIGNATURE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::RootSignature(HostileRootSignature::new())
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
    fn create_shader_resource_view(
        &self,
        resource: Option<&<Self::Api as direct3d_12::Direct3D12>::Resource>,
        desc: Option<&direct3d_12::structures::ShaderResourceViewDesc>,
        dest_descriptor: direct3d_12::structures::CpuDescriptorHandle,
    ) {
        let _ = &resource;
        let _ = &desc;
        let _ = &dest_descriptor;
        record("ID3D12Device::CreateShaderResourceView()".to_string());
    }
    fn create_render_target_view(
        &self,
        resource: Option<&<Self::Api as direct3d_12::Direct3D12>::Resource>,
        desc: Option<&direct3d_12::structures::RenderTargetViewDesc>,
        dest_descriptor: direct3d_12::structures::CpuDescriptorHandle,
    ) {
        let _ = &resource;
        let _ = &desc;
        let _ = &dest_descriptor;
        record("ID3D12Device::CreateRenderTargetView()".to_string());
    }
    fn create_committed_resource(
        &self,
        heap_properties: &direct3d_12::structures::HeapProperties,
        heap_flags: direct3d_12::enumerations::HeapFlags,
        desc: &direct3d_12::structures::ResourceDesc,
        initial_resource_state: direct3d_12::enumerations::ResourceStates,
        optimized_clear_value: Option<&direct3d_12::structures::ClearValue>,
        riid_resource: &component_object_model::structures::Guid,
    ) -> Result<direct3d_12::UnknownObject<Self::Api>, component_object_model::structures::HResult>
    {
        let _ = &heap_properties;
        let _ = &heap_flags;
        let _ = &desc;
        let _ = &initial_resource_state;
        let _ = &optimized_clear_value;
        let _ = &riid_resource;
        record(format!(
            "ID3D12Device::CreateCommittedResource(heap_flags={:?}, initial_resource_state={:?})",
            heap_flags, initial_resource_state
        ));
        Ok(
            if [
                direct3d_12::constants::IID_ID3D12_COMMAND_ALLOCATOR,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid_resource)
            {
                direct3d_12::UnknownObject::CommandAllocator(HostileCommandAllocator::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_COMMAND_QUEUE,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid_resource)
            {
                direct3d_12::UnknownObject::CommandQueue(HostileCommandQueue::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DESCRIPTOR_HEAP,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid_resource)
            {
                direct3d_12::UnknownObject::DescriptorHeap(HostileDescriptorHeap::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE,
                direct3d_12::constants::IID_ID3D12_OBJECT,
            ]
            .contains(riid_resource)
            {
                direct3d_12::UnknownObject::Device(HostileDevice::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_FENCE,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid_resource)
            {
                direct3d_12::UnknownObject::Fence(HostileFence::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_COMMAND_LIST,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_GRAPHICS_COMMAND_LIST,
                direct3d_12::constants::IID_ID3D12_OBJECT,
            ]
            .contains(riid_resource)
            {
                direct3d_12::UnknownObject::GraphicsCommandList(HostileGraphicsCommandList::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
                direct3d_12::constants::IID_ID3D12_PIPELINE_STATE,
            ]
            .contains(riid_resource)
            {
                direct3d_12::UnknownObject::PipelineState(HostilePipelineState::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
                direct3d_12::constants::IID_ID3D12_QUERY_HEAP,
            ]
            .contains(riid_resource)
            {
                direct3d_12::UnknownObject::QueryHeap(HostileQueryHeap::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
                direct3d_12::constants::IID_ID3D12_RESOURCE,
            ]
            .contains(riid_resource)
            {
                direct3d_12::UnknownObject::Resource(HostileResource::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_ROOT_SIGNATURE,
            ]
            .contains(riid_resource)
            {
                direct3d_12::UnknownObject::RootSignature(HostileRootSignature::new())
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
    fn create_fence(
        &self,
        initial_value: u64,
        flags: direct3d_12::enumerations::FenceFlags,
        riid: &component_object_model::structures::Guid,
    ) -> Result<direct3d_12::UnknownObject<Self::Api>, component_object_model::structures::HResult>
    {
        let _ = &initial_value;
        let _ = &flags;
        let _ = &riid;
        record(format!(
            "ID3D12Device::CreateFence(initial_value={:?}, flags={:?})",
            initial_value, flags
        ));
        Ok(
            if [
                direct3d_12::constants::IID_ID3D12_COMMAND_ALLOCATOR,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::CommandAllocator(HostileCommandAllocator::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_COMMAND_QUEUE,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::CommandQueue(HostileCommandQueue::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DESCRIPTOR_HEAP,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::DescriptorHeap(HostileDescriptorHeap::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE,
                direct3d_12::constants::IID_ID3D12_OBJECT,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::Device(HostileDevice::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_FENCE,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::Fence(HostileFence::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_COMMAND_LIST,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_GRAPHICS_COMMAND_LIST,
                direct3d_12::constants::IID_ID3D12_OBJECT,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::GraphicsCommandList(HostileGraphicsCommandList::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
                direct3d_12::constants::IID_ID3D12_PIPELINE_STATE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::PipelineState(HostilePipelineState::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
                direct3d_12::constants::IID_ID3D12_QUERY_HEAP,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::QueryHeap(HostileQueryHeap::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
                direct3d_12::constants::IID_ID3D12_RESOURCE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::Resource(HostileResource::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_ROOT_SIGNATURE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::RootSignature(HostileRootSignature::new())
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
    fn get_copyable_footprints(
        &self,
        resource_desc: &direct3d_12::structures::ResourceDesc,
        first_subresource: u32,
        num_subresources: u32,
        base_offset: u64,
    ) -> (
        Vec<direct3d_12::structures::PlacedSubresourceFootprint>,
        Vec<u32>,
        Vec<u64>,
        u64,
    ) {
        let _ = &resource_desc;
        let _ = &first_subresource;
        let _ = &num_subresources;
        let _ = &base_offset;
        record(format!(
            "ID3D12Device::GetCopyableFootprints(first_subresource={:?}, num_subresources={:?}, base_offset={:?})",
            first_subresource, num_subresources, base_offset
        ));
        ((0..num_subresources.min(4)).map(|_| data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_placed_subresource_footprint(&[u8::MIN; 32])).collect(), (0..num_subresources.min(4)).map(|_| Default::default()).collect(), (0..num_subresources.min(4)).map(|_| Default::default()).collect(), Default::default())
    }
    fn create_query_heap(
        &self,
        desc: &direct3d_12::structures::QueryHeapDesc,
        riid: &component_object_model::structures::Guid,
    ) -> Result<direct3d_12::UnknownObject<Self::Api>, component_object_model::structures::HResult>
    {
        let _ = &desc;
        let _ = &riid;
        record("ID3D12Device::CreateQueryHeap()".to_string());
        Ok(
            if [
                direct3d_12::constants::IID_ID3D12_COMMAND_ALLOCATOR,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::CommandAllocator(HostileCommandAllocator::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_COMMAND_QUEUE,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::CommandQueue(HostileCommandQueue::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DESCRIPTOR_HEAP,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::DescriptorHeap(HostileDescriptorHeap::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE,
                direct3d_12::constants::IID_ID3D12_OBJECT,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::Device(HostileDevice::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_FENCE,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::Fence(HostileFence::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_COMMAND_LIST,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_GRAPHICS_COMMAND_LIST,
                direct3d_12::constants::IID_ID3D12_OBJECT,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::GraphicsCommandList(HostileGraphicsCommandList::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
                direct3d_12::constants::IID_ID3D12_PIPELINE_STATE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::PipelineState(HostilePipelineState::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
                direct3d_12::constants::IID_ID3D12_QUERY_HEAP,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::QueryHeap(HostileQueryHeap::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
                direct3d_12::constants::IID_ID3D12_RESOURCE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::Resource(HostileResource::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_ROOT_SIGNATURE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::RootSignature(HostileRootSignature::new())
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
}

impl<A: Hostile> direct3d_12::Object for HostileDevice<A> {
    type Api = A;
}

impl<A: Hostile> direct3d_12::Fence for HostileFence<A> {
    fn get_completed_value(&self) -> u64 {
        record("ID3D12Fence::GetCompletedValue()".to_string());
        Default::default()
    }
    fn set_event_on_completion(
        &self,
        value: u64,
        h_event: component_object_model::structures::Handle,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &value;
        let _ = &h_event;
        record(format!(
            "ID3D12Fence::SetEventOnCompletion(value={:?}, h_event={:?})",
            value, h_event
        ));
        Ok(())
    }
}

impl<A: Hostile> direct3d_12::Pageable for HostileFence<A> {}

impl<A: Hostile> direct3d_12::DeviceChild for HostileFence<A> {}

impl<A: Hostile> direct3d_12::Object for HostileFence<A> {
    type Api = A;
}

impl<A: Hostile> direct3d_12::GraphicsCommandList for HostileGraphicsCommandList<A> {
    fn close(&self) -> Result<(), component_object_model::structures::HResult> {
        record("ID3D12GraphicsCommandList::Close()".to_string());
        Ok(())
    }
    fn reset(
        &self,
        allocator: &<Self::Api as direct3d_12::Direct3D12>::CommandAllocator,
        initial_state: Option<&<Self::Api as direct3d_12::Direct3D12>::PipelineState>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &allocator;
        let _ = &initial_state;
        record("ID3D12GraphicsCommandList::Reset()".to_string());
        Ok(())
    }
    fn draw_instanced(
        &self,
        vertex_count_per_instance: u32,
        instance_count: u32,
        start_vertex_location: u32,
        start_instance_location: u32,
    ) {
        let _ = &vertex_count_per_instance;
        let _ = &instance_count;
        let _ = &start_vertex_location;
        let _ = &start_instance_location;
        record(format!(
            "ID3D12GraphicsCommandList::DrawInstanced(vertex_count_per_instance={:?}, instance_count={:?}, start_vertex_location={:?}, start_instance_location={:?})",
            vertex_count_per_instance,
            instance_count,
            start_vertex_location,
            start_instance_location
        ));
    }
    fn draw_indexed_instanced(
        &self,
        index_count_per_instance: u32,
        instance_count: u32,
        start_index_location: u32,
        base_vertex_location: i32,
        start_instance_location: u32,
    ) {
        let _ = &index_count_per_instance;
        let _ = &instance_count;
        let _ = &start_index_location;
        let _ = &base_vertex_location;
        let _ = &start_instance_location;
        record(format!(
            "ID3D12GraphicsCommandList::DrawIndexedInstanced(index_count_per_instance={:?}, instance_count={:?}, start_index_location={:?}, base_vertex_location={:?}, start_instance_location={:?})",
            index_count_per_instance,
            instance_count,
            start_index_location,
            base_vertex_location,
            start_instance_location
        ));
    }
    fn dispatch(
        &self,
        thread_group_count_x: u32,
        thread_group_count_y: u32,
        thread_group_count_z: u32,
    ) {
        let _ = &thread_group_count_x;
        let _ = &thread_group_count_y;
        let _ = &thread_group_count_z;
        record(format!(
            "ID3D12GraphicsCommandList::Dispatch(thread_group_count_x={:?}, thread_group_count_y={:?}, thread_group_count_z={:?})",
            thread_group_count_x, thread_group_count_y, thread_group_count_z
        ));
    }
    fn copy_buffer_region(
        &self,
        dst_buffer: &<Self::Api as direct3d_12::Direct3D12>::Resource,
        dst_offset: u64,
        src_buffer: &<Self::Api as direct3d_12::Direct3D12>::Resource,
        src_offset: u64,
        num_bytes: u64,
    ) {
        let _ = &dst_buffer;
        let _ = &dst_offset;
        let _ = &src_buffer;
        let _ = &src_offset;
        let _ = &num_bytes;
        record(format!(
            "ID3D12GraphicsCommandList::CopyBufferRegion(dst_offset={:?}, src_offset={:?}, num_bytes={:?})",
            dst_offset, src_offset, num_bytes
        ));
    }
    fn copy_texture_region(
        &self,
        dst: &direct3d_12::structures::TextureCopyLocation<Self::Api>,
        dst_x: u32,
        dst_y: u32,
        dst_z: u32,
        src: &direct3d_12::structures::TextureCopyLocation<Self::Api>,
        src_box: Option<&direct3d_12::structures::Box>,
    ) {
        let _ = &dst;
        let _ = &dst_x;
        let _ = &dst_y;
        let _ = &dst_z;
        let _ = &src;
        let _ = &src_box;
        record("ID3D12GraphicsCommandList::CopyTextureRegion()".to_string());
    }
    fn ia_set_primitive_topology(
        &self,
        primitive_topology: direct3d_common::enumerations::PrimitiveTopology,
    ) {
        let _ = &primitive_topology;
        record(format!(
            "ID3D12GraphicsCommandList::IASetPrimitiveTopology(primitive_topology={:?})",
            primitive_topology
        ));
    }
    fn set_pipeline_state(
        &self,
        pipeline_state: &<Self::Api as direct3d_12::Direct3D12>::PipelineState,
    ) {
        let _ = &pipeline_state;
        record("ID3D12GraphicsCommandList::SetPipelineState()".to_string());
    }
    fn resource_barrier(&self, barriers: &[direct3d_12::structures::ResourceBarrier<Self::Api>]) {
        let _ = &barriers;
        record("ID3D12GraphicsCommandList::ResourceBarrier()".to_string());
    }
    fn set_descriptor_heaps(
        &self,
        descriptor_heaps: &[Option<&<Self::Api as direct3d_12::Direct3D12>::DescriptorHeap>],
    ) {
        let _ = &descriptor_heaps;
        record("ID3D12GraphicsCommandList::SetDescriptorHeaps()".to_string());
    }
    fn set_compute_root_signature(
        &self,
        root_signature: Option<&<Self::Api as direct3d_12::Direct3D12>::RootSignature>,
    ) {
        let _ = &root_signature;
        record("ID3D12GraphicsCommandList::SetComputeRootSignature()".to_string());
    }
    fn set_graphics_root_signature(
        &self,
        root_signature: Option<&<Self::Api as direct3d_12::Direct3D12>::RootSignature>,
    ) {
        let _ = &root_signature;
        record("ID3D12GraphicsCommandList::SetGraphicsRootSignature()".to_string());
    }
    fn set_graphics_root_descriptor_table(
        &self,
        root_parameter_index: u32,
        base_descriptor: direct3d_12::structures::GpuDescriptorHandle,
    ) {
        let _ = &root_parameter_index;
        let _ = &base_descriptor;
        record("ID3D12GraphicsCommandList::SetGraphicsRootDescriptorTable()".to_string());
    }
    fn set_compute_root_constant_buffer_view(
        &self,
        root_parameter_index: u32,
        buffer_location: u64,
    ) {
        let _ = &root_parameter_index;
        let _ = &buffer_location;
        record("ID3D12GraphicsCommandList::SetComputeRootConstantBufferView()".to_string());
    }
    fn set_graphics_root_constant_buffer_view(
        &self,
        root_parameter_index: u32,
        buffer_location: u64,
    ) {
        let _ = &root_parameter_index;
        let _ = &buffer_location;
        record("ID3D12GraphicsCommandList::SetGraphicsRootConstantBufferView()".to_string());
    }
    fn set_compute_root_unordered_access_view(
        &self,
        root_parameter_index: u32,
        buffer_location: u64,
    ) {
        let _ = &root_parameter_index;
        let _ = &buffer_location;
        record("ID3D12GraphicsCommandList::SetComputeRootUnorderedAccessView()".to_string());
    }
    fn ia_set_index_buffer(&self, view: Option<&direct3d_12::structures::IndexBufferView>) {
        let _ = &view;
        record("ID3D12GraphicsCommandList::IASetIndexBuffer()".to_string());
    }
    fn ia_set_vertex_buffers(
        &self,
        start_slot: u32,
        views: &[direct3d_12::structures::VertexBufferView],
    ) {
        let _ = &start_slot;
        let _ = &views;
        record("ID3D12GraphicsCommandList::IASetVertexBuffers()".to_string());
    }
    fn so_set_targets(
        &self,
        start_slot: u32,
        views: &[direct3d_12::structures::StreamOutputBufferView],
    ) {
        let _ = &start_slot;
        let _ = &views;
        record("ID3D12GraphicsCommandList::SOSetTargets()".to_string());
    }
    fn om_set_render_targets(
        &self,
        num_render_target_descriptors: u32,
        render_target_descriptors: Option<&direct3d_12::structures::CpuDescriptorHandle>,
        r_ts_single_handle_to_descriptor_range: bool,
        depth_stencil_descriptor: Option<&direct3d_12::structures::CpuDescriptorHandle>,
    ) {
        let _ = &num_render_target_descriptors;
        let _ = &render_target_descriptors;
        let _ = &r_ts_single_handle_to_descriptor_range;
        let _ = &depth_stencil_descriptor;
        record("ID3D12GraphicsCommandList::OMSetRenderTargets()".to_string());
    }
    fn clear_render_target_view(
        &self,
        render_target_view: direct3d_12::structures::CpuDescriptorHandle,
        color_rgba: &[f32; direct3d_12::constants::GRAPHICS_COMMAND_LIST_CLEAR_RENDER_TARGET_VIEW_COLOR_RGBA_LENGTH],
        rects: &[component_object_model::structures::Rect],
    ) {
        let _ = &render_target_view;
        let _ = &color_rgba;
        let _ = &rects;
        record("ID3D12GraphicsCommandList::ClearRenderTargetView()".to_string());
    }
    fn begin_query(
        &self,
        query_heap: &<Self::Api as direct3d_12::Direct3D12>::QueryHeap,
        type_: direct3d_12::enumerations::QueryType,
        index: u32,
    ) {
        let _ = &query_heap;
        let _ = &type_;
        let _ = &index;
        record(format!(
            "ID3D12GraphicsCommandList::BeginQuery(type_={:?}, index={:?})",
            type_, index
        ));
    }
    fn end_query(
        &self,
        query_heap: &<Self::Api as direct3d_12::Direct3D12>::QueryHeap,
        type_: direct3d_12::enumerations::QueryType,
        index: u32,
    ) {
        let _ = &query_heap;
        let _ = &type_;
        let _ = &index;
        record(format!(
            "ID3D12GraphicsCommandList::EndQuery(type_={:?}, index={:?})",
            type_, index
        ));
    }
    fn resolve_query_data(
        &self,
        query_heap: &<Self::Api as direct3d_12::Direct3D12>::QueryHeap,
        type_: direct3d_12::enumerations::QueryType,
        start_index: u32,
        num_queries: u32,
        destination_buffer: &<Self::Api as direct3d_12::Direct3D12>::Resource,
        aligned_destination_buffer_offset: u64,
    ) {
        let _ = &query_heap;
        let _ = &type_;
        let _ = &start_index;
        let _ = &num_queries;
        let _ = &destination_buffer;
        let _ = &aligned_destination_buffer_offset;
        record(format!(
            "ID3D12GraphicsCommandList::ResolveQueryData(type_={:?}, start_index={:?}, num_queries={:?}, aligned_destination_buffer_offset={:?})",
            type_, start_index, num_queries, aligned_destination_buffer_offset
        ));
    }
}

impl<A: Hostile> direct3d_12::CommandList for HostileGraphicsCommandList<A> {}

impl<A: Hostile> direct3d_12::DeviceChild for HostileGraphicsCommandList<A> {}

impl<A: Hostile> direct3d_12::Object for HostileGraphicsCommandList<A> {
    type Api = A;
}

impl<A: Hostile> direct3d_12::PipelineState for HostilePipelineState<A> {}

impl<A: Hostile> direct3d_12::Pageable for HostilePipelineState<A> {}

impl<A: Hostile> direct3d_12::DeviceChild for HostilePipelineState<A> {}

impl<A: Hostile> direct3d_12::Object for HostilePipelineState<A> {
    type Api = A;
}

impl<A: Hostile> direct3d_12::QueryHeap for HostileQueryHeap<A> {}

impl<A: Hostile> direct3d_12::Pageable for HostileQueryHeap<A> {}

impl<A: Hostile> direct3d_12::DeviceChild for HostileQueryHeap<A> {}

impl<A: Hostile> direct3d_12::Object for HostileQueryHeap<A> {
    type Api = A;
}

impl<A: Hostile> direct3d_12::Resource for HostileResource<A> {
    fn map(
        &self,
        subresource: u32,
        read_range: Option<&direct3d_12::structures::Range>,
        data: &impl wcontainers_volatile_bytes::VolatileBytes,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = &subresource;
        let _ = &read_range;
        let _ = &data;
        record("ID3D12Resource::Map()".to_string());
        touch(data);
        Err(E_NOTIMPL)
    }
    fn unmap(&self, subresource: u32, written_range: Option<&direct3d_12::structures::Range>) {
        let _ = &subresource;
        let _ = &written_range;
        record(format!(
            "ID3D12Resource::Unmap(subresource={:?})",
            subresource
        ));
    }
    fn get_desc(&self) -> direct3d_12::structures::ResourceDesc {
        record("ID3D12Resource::GetDesc()".to_string());
        data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_resource_desc(
            &[u8::MIN; 56],
        )
    }
    fn get_gpu_virtual_address(&self) -> u64 {
        record("ID3D12Resource::GetGPUVirtualAddress()".to_string());
        Default::default()
    }
}

impl<A: Hostile> direct3d_12::Pageable for HostileResource<A> {}

impl<A: Hostile> direct3d_12::DeviceChild for HostileResource<A> {}

impl<A: Hostile> direct3d_12::Object for HostileResource<A> {
    type Api = A;
}

impl<A: Hostile> direct3d_12::RootSignature for HostileRootSignature<A> {}

impl<A: Hostile> direct3d_12::DeviceChild for HostileRootSignature<A> {}

impl<A: Hostile> direct3d_12::Object for HostileRootSignature<A> {
    type Api = A;
}

/// The hostile implementation's library: `Direct3D12` and every API it extends.
#[derive(Clone, Copy, Debug, Default)]
pub struct HostileLibrary;

impl direct3d_12::Direct3D12 for HostileLibrary {
    type CommandAllocator = HostileCommandAllocator<Self>;
    type CommandQueue = HostileCommandQueue<Self>;
    type DescriptorHeap = HostileDescriptorHeap<Self>;
    type Device = HostileDevice<Self>;
    type Fence = HostileFence<Self>;
    type GraphicsCommandList = HostileGraphicsCommandList<Self>;
    type PipelineState = HostilePipelineState<Self>;
    type QueryHeap = HostileQueryHeap<Self>;
    type Resource = HostileResource<Self>;
    type RootSignature = HostileRootSignature<Self>;

    fn create_device(
        &self,
        adapter: Option<direct3d_12::UnknownRef<'_, Self>>,
        minimum_feature_level: direct3d_common::enumerations::FeatureLevel,
        riid: &component_object_model::structures::Guid,
    ) -> Result<direct3d_12::UnknownObject<Self>, component_object_model::structures::HResult> {
        let _ = &adapter;
        let _ = &minimum_feature_level;
        let _ = &riid;
        record(format!(
            "D3D12CreateDevice(minimum_feature_level={:?})",
            minimum_feature_level
        ));
        Ok(
            if [
                direct3d_12::constants::IID_ID3D12_COMMAND_ALLOCATOR,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::CommandAllocator(HostileCommandAllocator::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_COMMAND_QUEUE,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::CommandQueue(HostileCommandQueue::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DESCRIPTOR_HEAP,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::DescriptorHeap(HostileDescriptorHeap::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE,
                direct3d_12::constants::IID_ID3D12_OBJECT,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::Device(HostileDevice::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_FENCE,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::Fence(HostileFence::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_COMMAND_LIST,
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_GRAPHICS_COMMAND_LIST,
                direct3d_12::constants::IID_ID3D12_OBJECT,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::GraphicsCommandList(HostileGraphicsCommandList::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
                direct3d_12::constants::IID_ID3D12_PIPELINE_STATE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::PipelineState(HostilePipelineState::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
                direct3d_12::constants::IID_ID3D12_QUERY_HEAP,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::QueryHeap(HostileQueryHeap::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_PAGEABLE,
                direct3d_12::constants::IID_ID3D12_RESOURCE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::Resource(HostileResource::new())
            } else if [
                direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
                direct3d_12::constants::IID_ID3D12_OBJECT,
                direct3d_12::constants::IID_ID3D12_ROOT_SIGNATURE,
            ]
            .contains(riid)
            {
                direct3d_12::UnknownObject::RootSignature(HostileRootSignature::new())
            } else {
                return Err(E_NOINTERFACE);
            },
        )
    }
}

impl Hostile for HostileLibrary {}

impl direct3d_common::Direct3DCommon for HostileLibrary {
    type Blob = direct3d_common_hostile::HostileBlob<Self>;
    type DestructionNotifier = direct3d_common_hostile::HostileDestructionNotifier<Self>;
}

impl direct3d_common_hostile::Hostile for HostileLibrary {}

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
pub type Api = direct3d_12_direct::SystemDirect3D12;

/// One object of each kind the driver made, by kind.
pub struct Objects {
    pub api: Api,
    /// `Blob`.
    pub k0: Option<direct3d_common_direct::SystemBlob<Api>>,
    /// `DestructionNotifier`.
    pub k1: Option<direct3d_common_direct::SystemDestructionNotifier<Api>>,
    /// `CommandAllocator`.
    pub k51: Option<direct3d_12_direct::SystemCommandAllocator<Api>>,
    /// `CommandQueue`.
    pub k52: Option<direct3d_12_direct::SystemCommandQueue<Api>>,
    /// `DescriptorHeap`.
    pub k53: Option<direct3d_12_direct::SystemDescriptorHeap<Api>>,
    /// `Device`.
    pub k54: Option<direct3d_12_direct::SystemDevice<Api>>,
    /// `Fence`.
    pub k55: Option<direct3d_12_direct::SystemFence<Api>>,
    /// `GraphicsCommandList`.
    pub k56: Option<direct3d_12_direct::SystemGraphicsCommandList<Api>>,
    /// `PipelineState`.
    pub k57: Option<direct3d_12_direct::SystemPipelineState<Api>>,
    /// `QueryHeap`.
    pub k58: Option<direct3d_12_direct::SystemQueryHeap<Api>>,
    /// `Resource`.
    pub k59: Option<direct3d_12_direct::SystemResource<Api>>,
    /// `RootSignature`.
    pub k60: Option<direct3d_12_direct::SystemRootSignature<Api>>,
}

impl Objects {
    /// No object yet, and the API served by the serving glue over the hostile implementation.
    pub fn new() -> Objects {
        Objects {
            api: direct3d_12_direct::served(HostileLibrary),
            k0: None,
            k1: None,
            k51: None,
            k52: None,
            k53: None,
            k54: None,
            k55: None,
            k56: None,
            k57: None,
            k58: None,
            k59: None,
            k60: None,
        }
    }

    /// How many kinds have an object.
    pub fn kinds_held(&self) -> usize {
        [
            self.k0.is_some(),
            self.k1.is_some(),
            self.k51.is_some(),
            self.k52.is_some(),
            self.k53.is_some(),
            self.k54.is_some(),
            self.k55.is_some(),
            self.k56.is_some(),
            self.k57.is_some(),
            self.k58.is_some(),
            self.k59.is_some(),
            self.k60.is_some(),
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
    /// The `CommandAllocator` held, if any.
    pub fn command_allocator(&self) -> Option<&direct3d_12_direct::SystemCommandAllocator<Api>> {
        self.k51.as_ref()
    }

    /// Holds a `CommandAllocator`, in place of any held.
    pub fn put_command_allocator(
        &mut self,
        value: direct3d_12_direct::SystemCommandAllocator<Api>,
    ) {
        self.k51 = Some(value);
    }

    /// The `CommandQueue` held, if any.
    pub fn command_queue(&self) -> Option<&direct3d_12_direct::SystemCommandQueue<Api>> {
        self.k52.as_ref()
    }

    /// Holds a `CommandQueue`, in place of any held.
    pub fn put_command_queue(&mut self, value: direct3d_12_direct::SystemCommandQueue<Api>) {
        self.k52 = Some(value);
    }

    /// The `DescriptorHeap` held, if any.
    pub fn descriptor_heap(&self) -> Option<&direct3d_12_direct::SystemDescriptorHeap<Api>> {
        self.k53.as_ref()
    }

    /// Holds a `DescriptorHeap`, in place of any held.
    pub fn put_descriptor_heap(&mut self, value: direct3d_12_direct::SystemDescriptorHeap<Api>) {
        self.k53 = Some(value);
    }

    /// The `Device` held, if any.
    pub fn device(&self) -> Option<&direct3d_12_direct::SystemDevice<Api>> {
        self.k54.as_ref()
    }

    /// Holds a `Device`, in place of any held.
    pub fn put_device(&mut self, value: direct3d_12_direct::SystemDevice<Api>) {
        self.k54 = Some(value);
    }

    /// The `Fence` held, if any.
    pub fn fence(&self) -> Option<&direct3d_12_direct::SystemFence<Api>> {
        self.k55.as_ref()
    }

    /// Holds a `Fence`, in place of any held.
    pub fn put_fence(&mut self, value: direct3d_12_direct::SystemFence<Api>) {
        self.k55 = Some(value);
    }

    /// The `GraphicsCommandList` held, if any.
    pub fn graphics_command_list(
        &self,
    ) -> Option<&direct3d_12_direct::SystemGraphicsCommandList<Api>> {
        self.k56.as_ref()
    }

    /// Holds a `GraphicsCommandList`, in place of any held.
    pub fn put_graphics_command_list(
        &mut self,
        value: direct3d_12_direct::SystemGraphicsCommandList<Api>,
    ) {
        self.k56 = Some(value);
    }

    /// The `PipelineState` held, if any.
    pub fn pipeline_state(&self) -> Option<&direct3d_12_direct::SystemPipelineState<Api>> {
        self.k57.as_ref()
    }

    /// Holds a `PipelineState`, in place of any held.
    pub fn put_pipeline_state(&mut self, value: direct3d_12_direct::SystemPipelineState<Api>) {
        self.k57 = Some(value);
    }

    /// The `QueryHeap` held, if any.
    pub fn query_heap(&self) -> Option<&direct3d_12_direct::SystemQueryHeap<Api>> {
        self.k58.as_ref()
    }

    /// Holds a `QueryHeap`, in place of any held.
    pub fn put_query_heap(&mut self, value: direct3d_12_direct::SystemQueryHeap<Api>) {
        self.k58 = Some(value);
    }

    /// The `Resource` held, if any.
    pub fn resource(&self) -> Option<&direct3d_12_direct::SystemResource<Api>> {
        self.k59.as_ref()
    }

    /// Holds a `Resource`, in place of any held.
    pub fn put_resource(&mut self, value: direct3d_12_direct::SystemResource<Api>) {
        self.k59 = Some(value);
    }

    /// The `RootSignature` held, if any.
    pub fn root_signature(&self) -> Option<&direct3d_12_direct::SystemRootSignature<Api>> {
        self.k60.as_ref()
    }

    /// Holds a `RootSignature`, in place of any held.
    pub fn put_root_signature(&mut self, value: direct3d_12_direct::SystemRootSignature<Api>) {
        self.k60 = Some(value);
    }
}

/// Makes an object of every kind it can, by every method that hands one out, in rounds until none is new; returns how
/// many kinds hold one.
pub fn make_all(o: &mut Objects) -> usize {
    for _ in 0..6 {
        let before = o.kinds_held();
        // ID3D12Device::CreateCommandQueue for CommandAllocator
        if o.k51.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_command_queue(this, &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_command_queue_desc(&[u8::MIN; 16]), &direct3d_12::constants::IID_ID3D12_COMMAND_ALLOCATOR);
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::CommandAllocator(p0)) = answer
                && o.k51.is_none()
            {
                o.k51 = Some(p0);
            }
        }
        // ID3D12Device::CreateCommandQueue for CommandQueue
        if o.k52.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_command_queue(this, &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_command_queue_desc(&[u8::MIN; 16]), &direct3d_12::constants::IID_ID3D12_COMMAND_QUEUE);
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::CommandQueue(p0)) = answer
                && o.k52.is_none()
            {
                o.k52 = Some(p0);
            }
        }
        // ID3D12Device::CreateCommandQueue for DescriptorHeap
        if o.k53.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_command_queue(this, &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_command_queue_desc(&[u8::MIN; 16]), &direct3d_12::constants::IID_ID3D12_DESCRIPTOR_HEAP);
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::DescriptorHeap(p0)) = answer
                && o.k53.is_none()
            {
                o.k53 = Some(p0);
            }
        }
        // ID3D12Device::CreateCommandQueue for Device
        if o.k54.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_command_queue(this, &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_command_queue_desc(&[u8::MIN; 16]), &direct3d_12::constants::IID_ID3D12_DEVICE);
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::Device(p0)) = answer
                && o.k54.is_none()
            {
                o.k54 = Some(p0);
            }
        }
        // ID3D12Device::CreateCommandQueue for Fence
        if o.k55.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_command_queue(this, &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_command_queue_desc(&[u8::MIN; 16]), &direct3d_12::constants::IID_ID3D12_FENCE);
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::Fence(p0)) = answer
                && o.k55.is_none()
            {
                o.k55 = Some(p0);
            }
        }
        // ID3D12Device::CreateCommandQueue for GraphicsCommandList
        if o.k56.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_command_queue(this, &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_command_queue_desc(&[u8::MIN; 16]), &direct3d_12::constants::IID_ID3D12_GRAPHICS_COMMAND_LIST);
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::GraphicsCommandList(p0)) = answer
                && o.k56.is_none()
            {
                o.k56 = Some(p0);
            }
        }
        // ID3D12Device::CreateCommandQueue for PipelineState
        if o.k57.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_command_queue(this, &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_command_queue_desc(&[u8::MIN; 16]), &direct3d_12::constants::IID_ID3D12_PIPELINE_STATE);
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::PipelineState(p0)) = answer
                && o.k57.is_none()
            {
                o.k57 = Some(p0);
            }
        }
        // ID3D12Device::CreateCommandQueue for QueryHeap
        if o.k58.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_command_queue(this, &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_command_queue_desc(&[u8::MIN; 16]), &direct3d_12::constants::IID_ID3D12_QUERY_HEAP);
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::QueryHeap(p0)) = answer
                && o.k58.is_none()
            {
                o.k58 = Some(p0);
            }
        }
        // ID3D12Device::CreateCommandQueue for Resource
        if o.k59.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_command_queue(this, &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_command_queue_desc(&[u8::MIN; 16]), &direct3d_12::constants::IID_ID3D12_RESOURCE);
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::Resource(p0)) = answer
                && o.k59.is_none()
            {
                o.k59 = Some(p0);
            }
        }
        // ID3D12Device::CreateCommandQueue for RootSignature
        if o.k60.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_command_queue(this, &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_command_queue_desc(&[u8::MIN; 16]), &direct3d_12::constants::IID_ID3D12_ROOT_SIGNATURE);
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::RootSignature(p0)) = answer
                && o.k60.is_none()
            {
                o.k60 = Some(p0);
            }
        }
        // ID3D12Device::CreateCommandAllocator for CommandAllocator
        if o.k51.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_command_allocator(
                        this,
                        Default::default(),
                        &direct3d_12::constants::IID_ID3D12_COMMAND_ALLOCATOR,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::CommandAllocator(p0)) = answer
                && o.k51.is_none()
            {
                o.k51 = Some(p0);
            }
        }
        // ID3D12Device::CreateCommandAllocator for CommandQueue
        if o.k52.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_command_allocator(
                        this,
                        Default::default(),
                        &direct3d_12::constants::IID_ID3D12_COMMAND_QUEUE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::CommandQueue(p0)) = answer
                && o.k52.is_none()
            {
                o.k52 = Some(p0);
            }
        }
        // ID3D12Device::CreateCommandAllocator for DescriptorHeap
        if o.k53.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_command_allocator(
                        this,
                        Default::default(),
                        &direct3d_12::constants::IID_ID3D12_DESCRIPTOR_HEAP,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::DescriptorHeap(p0)) = answer
                && o.k53.is_none()
            {
                o.k53 = Some(p0);
            }
        }
        // ID3D12Device::CreateCommandAllocator for Device
        if o.k54.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_command_allocator(
                        this,
                        Default::default(),
                        &direct3d_12::constants::IID_ID3D12_DEVICE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::Device(p0)) = answer
                && o.k54.is_none()
            {
                o.k54 = Some(p0);
            }
        }
        // ID3D12Device::CreateCommandAllocator for Fence
        if o.k55.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_command_allocator(
                        this,
                        Default::default(),
                        &direct3d_12::constants::IID_ID3D12_FENCE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::Fence(p0)) = answer
                && o.k55.is_none()
            {
                o.k55 = Some(p0);
            }
        }
        // ID3D12Device::CreateCommandAllocator for GraphicsCommandList
        if o.k56.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_command_allocator(
                        this,
                        Default::default(),
                        &direct3d_12::constants::IID_ID3D12_GRAPHICS_COMMAND_LIST,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::GraphicsCommandList(p0)) = answer
                && o.k56.is_none()
            {
                o.k56 = Some(p0);
            }
        }
        // ID3D12Device::CreateCommandAllocator for PipelineState
        if o.k57.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_command_allocator(
                        this,
                        Default::default(),
                        &direct3d_12::constants::IID_ID3D12_PIPELINE_STATE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::PipelineState(p0)) = answer
                && o.k57.is_none()
            {
                o.k57 = Some(p0);
            }
        }
        // ID3D12Device::CreateCommandAllocator for QueryHeap
        if o.k58.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_command_allocator(
                        this,
                        Default::default(),
                        &direct3d_12::constants::IID_ID3D12_QUERY_HEAP,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::QueryHeap(p0)) = answer
                && o.k58.is_none()
            {
                o.k58 = Some(p0);
            }
        }
        // ID3D12Device::CreateCommandAllocator for Resource
        if o.k59.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_command_allocator(
                        this,
                        Default::default(),
                        &direct3d_12::constants::IID_ID3D12_RESOURCE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::Resource(p0)) = answer
                && o.k59.is_none()
            {
                o.k59 = Some(p0);
            }
        }
        // ID3D12Device::CreateCommandAllocator for RootSignature
        if o.k60.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_command_allocator(
                        this,
                        Default::default(),
                        &direct3d_12::constants::IID_ID3D12_ROOT_SIGNATURE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::RootSignature(p0)) = answer
                && o.k60.is_none()
            {
                o.k60 = Some(p0);
            }
        }
        // ID3D12Device::CreateCommandList for CommandAllocator
        if o.k51.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let v1 = o.k51.as_ref()?;
                    let answer = direct3d_12::Device::create_command_list(
                        this,
                        Default::default(),
                        Default::default(),
                        v1,
                        None,
                        &direct3d_12::constants::IID_ID3D12_COMMAND_ALLOCATOR,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::CommandAllocator(p0)) = answer
                && o.k51.is_none()
            {
                o.k51 = Some(p0);
            }
        }
        // ID3D12Device::CreateCommandList for CommandQueue
        if o.k52.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let v1 = o.k51.as_ref()?;
                    let answer = direct3d_12::Device::create_command_list(
                        this,
                        Default::default(),
                        Default::default(),
                        v1,
                        None,
                        &direct3d_12::constants::IID_ID3D12_COMMAND_QUEUE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::CommandQueue(p0)) = answer
                && o.k52.is_none()
            {
                o.k52 = Some(p0);
            }
        }
        // ID3D12Device::CreateCommandList for DescriptorHeap
        if o.k53.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let v1 = o.k51.as_ref()?;
                    let answer = direct3d_12::Device::create_command_list(
                        this,
                        Default::default(),
                        Default::default(),
                        v1,
                        None,
                        &direct3d_12::constants::IID_ID3D12_DESCRIPTOR_HEAP,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::DescriptorHeap(p0)) = answer
                && o.k53.is_none()
            {
                o.k53 = Some(p0);
            }
        }
        // ID3D12Device::CreateCommandList for Device
        if o.k54.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let v1 = o.k51.as_ref()?;
                    let answer = direct3d_12::Device::create_command_list(
                        this,
                        Default::default(),
                        Default::default(),
                        v1,
                        None,
                        &direct3d_12::constants::IID_ID3D12_DEVICE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::Device(p0)) = answer
                && o.k54.is_none()
            {
                o.k54 = Some(p0);
            }
        }
        // ID3D12Device::CreateCommandList for Fence
        if o.k55.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let v1 = o.k51.as_ref()?;
                    let answer = direct3d_12::Device::create_command_list(
                        this,
                        Default::default(),
                        Default::default(),
                        v1,
                        None,
                        &direct3d_12::constants::IID_ID3D12_FENCE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::Fence(p0)) = answer
                && o.k55.is_none()
            {
                o.k55 = Some(p0);
            }
        }
        // ID3D12Device::CreateCommandList for GraphicsCommandList
        if o.k56.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let v1 = o.k51.as_ref()?;
                    let answer = direct3d_12::Device::create_command_list(
                        this,
                        Default::default(),
                        Default::default(),
                        v1,
                        None,
                        &direct3d_12::constants::IID_ID3D12_GRAPHICS_COMMAND_LIST,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::GraphicsCommandList(p0)) = answer
                && o.k56.is_none()
            {
                o.k56 = Some(p0);
            }
        }
        // ID3D12Device::CreateCommandList for PipelineState
        if o.k57.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let v1 = o.k51.as_ref()?;
                    let answer = direct3d_12::Device::create_command_list(
                        this,
                        Default::default(),
                        Default::default(),
                        v1,
                        None,
                        &direct3d_12::constants::IID_ID3D12_PIPELINE_STATE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::PipelineState(p0)) = answer
                && o.k57.is_none()
            {
                o.k57 = Some(p0);
            }
        }
        // ID3D12Device::CreateCommandList for QueryHeap
        if o.k58.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let v1 = o.k51.as_ref()?;
                    let answer = direct3d_12::Device::create_command_list(
                        this,
                        Default::default(),
                        Default::default(),
                        v1,
                        None,
                        &direct3d_12::constants::IID_ID3D12_QUERY_HEAP,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::QueryHeap(p0)) = answer
                && o.k58.is_none()
            {
                o.k58 = Some(p0);
            }
        }
        // ID3D12Device::CreateCommandList for Resource
        if o.k59.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let v1 = o.k51.as_ref()?;
                    let answer = direct3d_12::Device::create_command_list(
                        this,
                        Default::default(),
                        Default::default(),
                        v1,
                        None,
                        &direct3d_12::constants::IID_ID3D12_RESOURCE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::Resource(p0)) = answer
                && o.k59.is_none()
            {
                o.k59 = Some(p0);
            }
        }
        // ID3D12Device::CreateCommandList for RootSignature
        if o.k60.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let v1 = o.k51.as_ref()?;
                    let answer = direct3d_12::Device::create_command_list(
                        this,
                        Default::default(),
                        Default::default(),
                        v1,
                        None,
                        &direct3d_12::constants::IID_ID3D12_ROOT_SIGNATURE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::RootSignature(p0)) = answer
                && o.k60.is_none()
            {
                o.k60 = Some(p0);
            }
        }
        // ID3D12Device::CreateDescriptorHeap for CommandAllocator
        if o.k51.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_descriptor_heap(this, &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_descriptor_heap_desc(&[u8::MIN; 16]), &direct3d_12::constants::IID_ID3D12_COMMAND_ALLOCATOR);
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::CommandAllocator(p0)) = answer
                && o.k51.is_none()
            {
                o.k51 = Some(p0);
            }
        }
        // ID3D12Device::CreateDescriptorHeap for CommandQueue
        if o.k52.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_descriptor_heap(this, &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_descriptor_heap_desc(&[u8::MIN; 16]), &direct3d_12::constants::IID_ID3D12_COMMAND_QUEUE);
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::CommandQueue(p0)) = answer
                && o.k52.is_none()
            {
                o.k52 = Some(p0);
            }
        }
        // ID3D12Device::CreateDescriptorHeap for DescriptorHeap
        if o.k53.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_descriptor_heap(this, &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_descriptor_heap_desc(&[u8::MIN; 16]), &direct3d_12::constants::IID_ID3D12_DESCRIPTOR_HEAP);
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::DescriptorHeap(p0)) = answer
                && o.k53.is_none()
            {
                o.k53 = Some(p0);
            }
        }
        // ID3D12Device::CreateDescriptorHeap for Device
        if o.k54.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_descriptor_heap(this, &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_descriptor_heap_desc(&[u8::MIN; 16]), &direct3d_12::constants::IID_ID3D12_DEVICE);
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::Device(p0)) = answer
                && o.k54.is_none()
            {
                o.k54 = Some(p0);
            }
        }
        // ID3D12Device::CreateDescriptorHeap for Fence
        if o.k55.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_descriptor_heap(this, &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_descriptor_heap_desc(&[u8::MIN; 16]), &direct3d_12::constants::IID_ID3D12_FENCE);
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::Fence(p0)) = answer
                && o.k55.is_none()
            {
                o.k55 = Some(p0);
            }
        }
        // ID3D12Device::CreateDescriptorHeap for GraphicsCommandList
        if o.k56.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_descriptor_heap(this, &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_descriptor_heap_desc(&[u8::MIN; 16]), &direct3d_12::constants::IID_ID3D12_GRAPHICS_COMMAND_LIST);
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::GraphicsCommandList(p0)) = answer
                && o.k56.is_none()
            {
                o.k56 = Some(p0);
            }
        }
        // ID3D12Device::CreateDescriptorHeap for PipelineState
        if o.k57.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_descriptor_heap(this, &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_descriptor_heap_desc(&[u8::MIN; 16]), &direct3d_12::constants::IID_ID3D12_PIPELINE_STATE);
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::PipelineState(p0)) = answer
                && o.k57.is_none()
            {
                o.k57 = Some(p0);
            }
        }
        // ID3D12Device::CreateDescriptorHeap for QueryHeap
        if o.k58.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_descriptor_heap(this, &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_descriptor_heap_desc(&[u8::MIN; 16]), &direct3d_12::constants::IID_ID3D12_QUERY_HEAP);
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::QueryHeap(p0)) = answer
                && o.k58.is_none()
            {
                o.k58 = Some(p0);
            }
        }
        // ID3D12Device::CreateDescriptorHeap for Resource
        if o.k59.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_descriptor_heap(this, &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_descriptor_heap_desc(&[u8::MIN; 16]), &direct3d_12::constants::IID_ID3D12_RESOURCE);
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::Resource(p0)) = answer
                && o.k59.is_none()
            {
                o.k59 = Some(p0);
            }
        }
        // ID3D12Device::CreateDescriptorHeap for RootSignature
        if o.k60.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_descriptor_heap(this, &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_descriptor_heap_desc(&[u8::MIN; 16]), &direct3d_12::constants::IID_ID3D12_ROOT_SIGNATURE);
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::RootSignature(p0)) = answer
                && o.k60.is_none()
            {
                o.k60 = Some(p0);
            }
        }
        // ID3D12Device::CreateRootSignature for CommandAllocator
        if o.k51.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_root_signature(
                        this,
                        Default::default(),
                        &[],
                        &direct3d_12::constants::IID_ID3D12_COMMAND_ALLOCATOR,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::CommandAllocator(p0)) = answer
                && o.k51.is_none()
            {
                o.k51 = Some(p0);
            }
        }
        // ID3D12Device::CreateRootSignature for CommandQueue
        if o.k52.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_root_signature(
                        this,
                        Default::default(),
                        &[],
                        &direct3d_12::constants::IID_ID3D12_COMMAND_QUEUE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::CommandQueue(p0)) = answer
                && o.k52.is_none()
            {
                o.k52 = Some(p0);
            }
        }
        // ID3D12Device::CreateRootSignature for DescriptorHeap
        if o.k53.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_root_signature(
                        this,
                        Default::default(),
                        &[],
                        &direct3d_12::constants::IID_ID3D12_DESCRIPTOR_HEAP,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::DescriptorHeap(p0)) = answer
                && o.k53.is_none()
            {
                o.k53 = Some(p0);
            }
        }
        // ID3D12Device::CreateRootSignature for Device
        if o.k54.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_root_signature(
                        this,
                        Default::default(),
                        &[],
                        &direct3d_12::constants::IID_ID3D12_DEVICE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::Device(p0)) = answer
                && o.k54.is_none()
            {
                o.k54 = Some(p0);
            }
        }
        // ID3D12Device::CreateRootSignature for Fence
        if o.k55.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_root_signature(
                        this,
                        Default::default(),
                        &[],
                        &direct3d_12::constants::IID_ID3D12_FENCE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::Fence(p0)) = answer
                && o.k55.is_none()
            {
                o.k55 = Some(p0);
            }
        }
        // ID3D12Device::CreateRootSignature for GraphicsCommandList
        if o.k56.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_root_signature(
                        this,
                        Default::default(),
                        &[],
                        &direct3d_12::constants::IID_ID3D12_GRAPHICS_COMMAND_LIST,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::GraphicsCommandList(p0)) = answer
                && o.k56.is_none()
            {
                o.k56 = Some(p0);
            }
        }
        // ID3D12Device::CreateRootSignature for PipelineState
        if o.k57.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_root_signature(
                        this,
                        Default::default(),
                        &[],
                        &direct3d_12::constants::IID_ID3D12_PIPELINE_STATE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::PipelineState(p0)) = answer
                && o.k57.is_none()
            {
                o.k57 = Some(p0);
            }
        }
        // ID3D12Device::CreateRootSignature for QueryHeap
        if o.k58.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_root_signature(
                        this,
                        Default::default(),
                        &[],
                        &direct3d_12::constants::IID_ID3D12_QUERY_HEAP,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::QueryHeap(p0)) = answer
                && o.k58.is_none()
            {
                o.k58 = Some(p0);
            }
        }
        // ID3D12Device::CreateRootSignature for Resource
        if o.k59.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_root_signature(
                        this,
                        Default::default(),
                        &[],
                        &direct3d_12::constants::IID_ID3D12_RESOURCE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::Resource(p0)) = answer
                && o.k59.is_none()
            {
                o.k59 = Some(p0);
            }
        }
        // ID3D12Device::CreateRootSignature for RootSignature
        if o.k60.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_root_signature(
                        this,
                        Default::default(),
                        &[],
                        &direct3d_12::constants::IID_ID3D12_ROOT_SIGNATURE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::RootSignature(p0)) = answer
                && o.k60.is_none()
            {
                o.k60 = Some(p0);
            }
        }
        // ID3D12Device::CreateCommittedResource for CommandAllocator
        if o.k51.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_committed_resource(this, &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_heap_properties(&[u8::MIN; 20]), Default::default(), &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_resource_desc(&[u8::MIN; 56]), Default::default(), None, &direct3d_12::constants::IID_ID3D12_COMMAND_ALLOCATOR);
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::CommandAllocator(p0)) = answer
                && o.k51.is_none()
            {
                o.k51 = Some(p0);
            }
        }
        // ID3D12Device::CreateCommittedResource for CommandQueue
        if o.k52.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_committed_resource(this, &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_heap_properties(&[u8::MIN; 20]), Default::default(), &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_resource_desc(&[u8::MIN; 56]), Default::default(), None, &direct3d_12::constants::IID_ID3D12_COMMAND_QUEUE);
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::CommandQueue(p0)) = answer
                && o.k52.is_none()
            {
                o.k52 = Some(p0);
            }
        }
        // ID3D12Device::CreateCommittedResource for DescriptorHeap
        if o.k53.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_committed_resource(this, &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_heap_properties(&[u8::MIN; 20]), Default::default(), &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_resource_desc(&[u8::MIN; 56]), Default::default(), None, &direct3d_12::constants::IID_ID3D12_DESCRIPTOR_HEAP);
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::DescriptorHeap(p0)) = answer
                && o.k53.is_none()
            {
                o.k53 = Some(p0);
            }
        }
        // ID3D12Device::CreateCommittedResource for Device
        if o.k54.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_committed_resource(this, &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_heap_properties(&[u8::MIN; 20]), Default::default(), &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_resource_desc(&[u8::MIN; 56]), Default::default(), None, &direct3d_12::constants::IID_ID3D12_DEVICE);
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::Device(p0)) = answer
                && o.k54.is_none()
            {
                o.k54 = Some(p0);
            }
        }
        // ID3D12Device::CreateCommittedResource for Fence
        if o.k55.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_committed_resource(this, &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_heap_properties(&[u8::MIN; 20]), Default::default(), &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_resource_desc(&[u8::MIN; 56]), Default::default(), None, &direct3d_12::constants::IID_ID3D12_FENCE);
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::Fence(p0)) = answer
                && o.k55.is_none()
            {
                o.k55 = Some(p0);
            }
        }
        // ID3D12Device::CreateCommittedResource for GraphicsCommandList
        if o.k56.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_committed_resource(this, &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_heap_properties(&[u8::MIN; 20]), Default::default(), &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_resource_desc(&[u8::MIN; 56]), Default::default(), None, &direct3d_12::constants::IID_ID3D12_GRAPHICS_COMMAND_LIST);
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::GraphicsCommandList(p0)) = answer
                && o.k56.is_none()
            {
                o.k56 = Some(p0);
            }
        }
        // ID3D12Device::CreateCommittedResource for PipelineState
        if o.k57.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_committed_resource(this, &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_heap_properties(&[u8::MIN; 20]), Default::default(), &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_resource_desc(&[u8::MIN; 56]), Default::default(), None, &direct3d_12::constants::IID_ID3D12_PIPELINE_STATE);
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::PipelineState(p0)) = answer
                && o.k57.is_none()
            {
                o.k57 = Some(p0);
            }
        }
        // ID3D12Device::CreateCommittedResource for QueryHeap
        if o.k58.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_committed_resource(this, &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_heap_properties(&[u8::MIN; 20]), Default::default(), &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_resource_desc(&[u8::MIN; 56]), Default::default(), None, &direct3d_12::constants::IID_ID3D12_QUERY_HEAP);
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::QueryHeap(p0)) = answer
                && o.k58.is_none()
            {
                o.k58 = Some(p0);
            }
        }
        // ID3D12Device::CreateCommittedResource for Resource
        if o.k59.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_committed_resource(this, &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_heap_properties(&[u8::MIN; 20]), Default::default(), &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_resource_desc(&[u8::MIN; 56]), Default::default(), None, &direct3d_12::constants::IID_ID3D12_RESOURCE);
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::Resource(p0)) = answer
                && o.k59.is_none()
            {
                o.k59 = Some(p0);
            }
        }
        // ID3D12Device::CreateCommittedResource for RootSignature
        if o.k60.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_committed_resource(this, &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_heap_properties(&[u8::MIN; 20]), Default::default(), &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_resource_desc(&[u8::MIN; 56]), Default::default(), None, &direct3d_12::constants::IID_ID3D12_ROOT_SIGNATURE);
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::RootSignature(p0)) = answer
                && o.k60.is_none()
            {
                o.k60 = Some(p0);
            }
        }
        // ID3D12Device::CreateFence for CommandAllocator
        if o.k51.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_12::constants::IID_ID3D12_COMMAND_ALLOCATOR,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::CommandAllocator(p0)) = answer
                && o.k51.is_none()
            {
                o.k51 = Some(p0);
            }
        }
        // ID3D12Device::CreateFence for CommandQueue
        if o.k52.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_12::constants::IID_ID3D12_COMMAND_QUEUE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::CommandQueue(p0)) = answer
                && o.k52.is_none()
            {
                o.k52 = Some(p0);
            }
        }
        // ID3D12Device::CreateFence for DescriptorHeap
        if o.k53.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_12::constants::IID_ID3D12_DESCRIPTOR_HEAP,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::DescriptorHeap(p0)) = answer
                && o.k53.is_none()
            {
                o.k53 = Some(p0);
            }
        }
        // ID3D12Device::CreateFence for Device
        if o.k54.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_12::constants::IID_ID3D12_DEVICE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::Device(p0)) = answer
                && o.k54.is_none()
            {
                o.k54 = Some(p0);
            }
        }
        // ID3D12Device::CreateFence for Fence
        if o.k55.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_12::constants::IID_ID3D12_FENCE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::Fence(p0)) = answer
                && o.k55.is_none()
            {
                o.k55 = Some(p0);
            }
        }
        // ID3D12Device::CreateFence for GraphicsCommandList
        if o.k56.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_12::constants::IID_ID3D12_GRAPHICS_COMMAND_LIST,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::GraphicsCommandList(p0)) = answer
                && o.k56.is_none()
            {
                o.k56 = Some(p0);
            }
        }
        // ID3D12Device::CreateFence for PipelineState
        if o.k57.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_12::constants::IID_ID3D12_PIPELINE_STATE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::PipelineState(p0)) = answer
                && o.k57.is_none()
            {
                o.k57 = Some(p0);
            }
        }
        // ID3D12Device::CreateFence for QueryHeap
        if o.k58.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_12::constants::IID_ID3D12_QUERY_HEAP,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::QueryHeap(p0)) = answer
                && o.k58.is_none()
            {
                o.k58 = Some(p0);
            }
        }
        // ID3D12Device::CreateFence for Resource
        if o.k59.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_12::constants::IID_ID3D12_RESOURCE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::Resource(p0)) = answer
                && o.k59.is_none()
            {
                o.k59 = Some(p0);
            }
        }
        // ID3D12Device::CreateFence for RootSignature
        if o.k60.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_fence(
                        this,
                        Default::default(),
                        Default::default(),
                        &direct3d_12::constants::IID_ID3D12_ROOT_SIGNATURE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::RootSignature(p0)) = answer
                && o.k60.is_none()
            {
                o.k60 = Some(p0);
            }
        }
        // ID3D12Device::CreateQueryHeap for CommandAllocator
        if o.k51.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_query_heap(this, &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_query_heap_desc(&[u8::MIN; 12]), &direct3d_12::constants::IID_ID3D12_COMMAND_ALLOCATOR);
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::CommandAllocator(p0)) = answer
                && o.k51.is_none()
            {
                o.k51 = Some(p0);
            }
        }
        // ID3D12Device::CreateQueryHeap for CommandQueue
        if o.k52.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_query_heap(this, &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_query_heap_desc(&[u8::MIN; 12]), &direct3d_12::constants::IID_ID3D12_COMMAND_QUEUE);
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::CommandQueue(p0)) = answer
                && o.k52.is_none()
            {
                o.k52 = Some(p0);
            }
        }
        // ID3D12Device::CreateQueryHeap for DescriptorHeap
        if o.k53.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_query_heap(this, &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_query_heap_desc(&[u8::MIN; 12]), &direct3d_12::constants::IID_ID3D12_DESCRIPTOR_HEAP);
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::DescriptorHeap(p0)) = answer
                && o.k53.is_none()
            {
                o.k53 = Some(p0);
            }
        }
        // ID3D12Device::CreateQueryHeap for Device
        if o.k54.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_query_heap(this, &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_query_heap_desc(&[u8::MIN; 12]), &direct3d_12::constants::IID_ID3D12_DEVICE);
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::Device(p0)) = answer
                && o.k54.is_none()
            {
                o.k54 = Some(p0);
            }
        }
        // ID3D12Device::CreateQueryHeap for Fence
        if o.k55.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_query_heap(this, &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_query_heap_desc(&[u8::MIN; 12]), &direct3d_12::constants::IID_ID3D12_FENCE);
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::Fence(p0)) = answer
                && o.k55.is_none()
            {
                o.k55 = Some(p0);
            }
        }
        // ID3D12Device::CreateQueryHeap for GraphicsCommandList
        if o.k56.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_query_heap(this, &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_query_heap_desc(&[u8::MIN; 12]), &direct3d_12::constants::IID_ID3D12_GRAPHICS_COMMAND_LIST);
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::GraphicsCommandList(p0)) = answer
                && o.k56.is_none()
            {
                o.k56 = Some(p0);
            }
        }
        // ID3D12Device::CreateQueryHeap for PipelineState
        if o.k57.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_query_heap(this, &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_query_heap_desc(&[u8::MIN; 12]), &direct3d_12::constants::IID_ID3D12_PIPELINE_STATE);
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::PipelineState(p0)) = answer
                && o.k57.is_none()
            {
                o.k57 = Some(p0);
            }
        }
        // ID3D12Device::CreateQueryHeap for QueryHeap
        if o.k58.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_query_heap(this, &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_query_heap_desc(&[u8::MIN; 12]), &direct3d_12::constants::IID_ID3D12_QUERY_HEAP);
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::QueryHeap(p0)) = answer
                && o.k58.is_none()
            {
                o.k58 = Some(p0);
            }
        }
        // ID3D12Device::CreateQueryHeap for Resource
        if o.k59.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_query_heap(this, &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_query_heap_desc(&[u8::MIN; 12]), &direct3d_12::constants::IID_ID3D12_RESOURCE);
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::Resource(p0)) = answer
                && o.k59.is_none()
            {
                o.k59 = Some(p0);
            }
        }
        // ID3D12Device::CreateQueryHeap for RootSignature
        if o.k60.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let this = o.k54.as_ref()?;
                    let answer = direct3d_12::Device::create_query_heap(this, &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_query_heap_desc(&[u8::MIN; 12]), &direct3d_12::constants::IID_ID3D12_ROOT_SIGNATURE);
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::RootSignature(p0)) = answer
                && o.k60.is_none()
            {
                o.k60 = Some(p0);
            }
        }
        // D3D12CreateDevice for CommandAllocator
        if o.k51.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = direct3d_12::Direct3D12::create_device(
                        &o.api,
                        None,
                        Default::default(),
                        &direct3d_12::constants::IID_ID3D12_COMMAND_ALLOCATOR,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::CommandAllocator(p0)) = answer
                && o.k51.is_none()
            {
                o.k51 = Some(p0);
            }
        }
        // D3D12CreateDevice for CommandQueue
        if o.k52.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = direct3d_12::Direct3D12::create_device(
                        &o.api,
                        None,
                        Default::default(),
                        &direct3d_12::constants::IID_ID3D12_COMMAND_QUEUE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::CommandQueue(p0)) = answer
                && o.k52.is_none()
            {
                o.k52 = Some(p0);
            }
        }
        // D3D12CreateDevice for DescriptorHeap
        if o.k53.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = direct3d_12::Direct3D12::create_device(
                        &o.api,
                        None,
                        Default::default(),
                        &direct3d_12::constants::IID_ID3D12_DESCRIPTOR_HEAP,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::DescriptorHeap(p0)) = answer
                && o.k53.is_none()
            {
                o.k53 = Some(p0);
            }
        }
        // D3D12CreateDevice for Device
        if o.k54.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = direct3d_12::Direct3D12::create_device(
                        &o.api,
                        None,
                        Default::default(),
                        &direct3d_12::constants::IID_ID3D12_DEVICE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::Device(p0)) = answer
                && o.k54.is_none()
            {
                o.k54 = Some(p0);
            }
        }
        // D3D12CreateDevice for Fence
        if o.k55.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = direct3d_12::Direct3D12::create_device(
                        &o.api,
                        None,
                        Default::default(),
                        &direct3d_12::constants::IID_ID3D12_FENCE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::Fence(p0)) = answer
                && o.k55.is_none()
            {
                o.k55 = Some(p0);
            }
        }
        // D3D12CreateDevice for GraphicsCommandList
        if o.k56.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = direct3d_12::Direct3D12::create_device(
                        &o.api,
                        None,
                        Default::default(),
                        &direct3d_12::constants::IID_ID3D12_GRAPHICS_COMMAND_LIST,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::GraphicsCommandList(p0)) = answer
                && o.k56.is_none()
            {
                o.k56 = Some(p0);
            }
        }
        // D3D12CreateDevice for PipelineState
        if o.k57.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = direct3d_12::Direct3D12::create_device(
                        &o.api,
                        None,
                        Default::default(),
                        &direct3d_12::constants::IID_ID3D12_PIPELINE_STATE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::PipelineState(p0)) = answer
                && o.k57.is_none()
            {
                o.k57 = Some(p0);
            }
        }
        // D3D12CreateDevice for QueryHeap
        if o.k58.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = direct3d_12::Direct3D12::create_device(
                        &o.api,
                        None,
                        Default::default(),
                        &direct3d_12::constants::IID_ID3D12_QUERY_HEAP,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::QueryHeap(p0)) = answer
                && o.k58.is_none()
            {
                o.k58 = Some(p0);
            }
        }
        // D3D12CreateDevice for Resource
        if o.k59.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = direct3d_12::Direct3D12::create_device(
                        &o.api,
                        None,
                        Default::default(),
                        &direct3d_12::constants::IID_ID3D12_RESOURCE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::Resource(p0)) = answer
                && o.k59.is_none()
            {
                o.k59 = Some(p0);
            }
        }
        // D3D12CreateDevice for RootSignature
        if o.k60.is_none() {
            let answer = {
                let o = &*o;
                made(|| {
                    let answer = direct3d_12::Direct3D12::create_device(
                        &o.api,
                        None,
                        Default::default(),
                        &direct3d_12::constants::IID_ID3D12_ROOT_SIGNATURE,
                    );
                    answer.ok()
                })
            };
            if let Some(direct3d_12::UnknownObject::RootSignature(p0)) = answer
                && o.k60.is_none()
            {
                o.k60 = Some(p0);
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
    // ID3DBlob::GetBufferSize on Blob
    if wanted(ID_3D_BLOB) {
        driven.attempt(|| {
            let Some(this) = o.k0.as_ref() else {
                return false;
            };
            let _ = direct3d_common::Blob::get_buffer_size(this);
            true
        });
    }
    // ID3DDestructionNotifier::UnregisterDestructionCallback on DestructionNotifier
    if wanted(ID_3D_DESTRUCTION_NOTIFIER) {
        driven.attempt(|| {
            let Some(this) = o.k1.as_ref() else {
                return false;
            };
            let _ = direct3d_common::DestructionNotifier::unregister_destruction_callback(
                this,
                Default::default(),
            );
            true
        });
    }
    // ID3D12CommandAllocator::Reset on CommandAllocator
    if wanted(ID_3D12_COMMAND_ALLOCATOR) {
        driven.attempt(|| {
            let Some(this) = o.k51.as_ref() else {
                return false;
            };
            let _ = direct3d_12::CommandAllocator::reset(this);
            true
        });
    }
    // ID3D12CommandQueue::Signal on CommandQueue
    if wanted(ID_3D12_COMMAND_QUEUE) {
        driven.attempt(|| {
            let Some(this) = o.k52.as_ref() else {
                return false;
            };
            let Some(v1) = o.k55.as_ref() else {
                return false;
            };
            let _ = direct3d_12::CommandQueue::signal(this, v1, Default::default());
            true
        });
    }
    // ID3D12DescriptorHeap::GetDesc on DescriptorHeap
    if wanted(ID_3D12_DESCRIPTOR_HEAP) {
        driven.attempt(|| {
            let Some(this) = o.k53.as_ref() else {
                return false;
            };
            let _ = direct3d_12::DescriptorHeap::get_desc(this);
            true
        });
    }
    // ID3D12Device::CreateCommandQueue on Device
    if wanted(ID_3D12_DEVICE) {
        driven.attempt(|| {
        let Some(this) = o.k54.as_ref() else { return false; };
        let _ = direct3d_12::Device::create_command_queue(this, &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_command_queue_desc(&[u8::MIN; 16]), &Default::default());
        true
        });
    }
    // ID3D12Device::CreateCommandAllocator on Device
    if wanted(ID_3D12_DEVICE) {
        driven.attempt(|| {
            let Some(this) = o.k54.as_ref() else {
                return false;
            };
            let _ = direct3d_12::Device::create_command_allocator(
                this,
                Default::default(),
                &Default::default(),
            );
            true
        });
    }
    // ID3D12Device::CreateCommandList on Device
    if wanted(ID_3D12_DEVICE) {
        driven.attempt(|| {
            let Some(this) = o.k54.as_ref() else {
                return false;
            };
            let Some(v1) = o.k51.as_ref() else {
                return false;
            };
            let _ = direct3d_12::Device::create_command_list(
                this,
                Default::default(),
                Default::default(),
                v1,
                None,
                &Default::default(),
            );
            true
        });
    }
    // ID3D12Device::CreateDescriptorHeap on Device
    if wanted(ID_3D12_DEVICE) {
        driven.attempt(|| {
        let Some(this) = o.k54.as_ref() else { return false; };
        let _ = direct3d_12::Device::create_descriptor_heap(this, &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_descriptor_heap_desc(&[u8::MIN; 16]), &Default::default());
        true
        });
    }
    // ID3D12Device::GetDescriptorHandleIncrementSize on Device
    if wanted(ID_3D12_DEVICE) {
        driven.attempt(|| {
            let Some(this) = o.k54.as_ref() else {
                return false;
            };
            let _ =
                direct3d_12::Device::get_descriptor_handle_increment_size(this, Default::default());
            true
        });
    }
    // ID3D12Device::CreateRootSignature on Device
    if wanted(ID_3D12_DEVICE) {
        driven.attempt(|| {
            let Some(this) = o.k54.as_ref() else {
                return false;
            };
            let _ = direct3d_12::Device::create_root_signature(
                this,
                Default::default(),
                &[],
                &Default::default(),
            );
            true
        });
    }
    // ID3D12Device::CreateCommittedResource on Device
    if wanted(ID_3D12_DEVICE) {
        driven.attempt(|| {
            let Some(this) = o.k54.as_ref() else {
                return false;
            };
            let _ = direct3d_12::Device::create_committed_resource(
                this,
                &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_heap_properties(
                    &[u8::MIN; 20],
                ),
                Default::default(),
                &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_resource_desc(
                    &[u8::MIN; 56],
                ),
                Default::default(),
                None,
                &Default::default(),
            );
            true
        });
    }
    // ID3D12Device::CreateFence on Device
    if wanted(ID_3D12_DEVICE) {
        driven.attempt(|| {
            let Some(this) = o.k54.as_ref() else {
                return false;
            };
            let _ = direct3d_12::Device::create_fence(
                this,
                Default::default(),
                Default::default(),
                &Default::default(),
            );
            true
        });
    }
    // ID3D12Device::GetCopyableFootprints on Device
    if wanted(ID_3D12_DEVICE) {
        driven.attempt(|| {
            let Some(this) = o.k54.as_ref() else {
                return false;
            };
            let _ = direct3d_12::Device::get_copyable_footprints(
                this,
                &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_resource_desc(
                    &[u8::MIN; 56],
                ),
                Default::default(),
                Default::default(),
                Default::default(),
            );
            true
        });
    }
    // ID3D12Device::CreateQueryHeap on Device
    if wanted(ID_3D12_DEVICE) {
        driven.attempt(|| {
            let Some(this) = o.k54.as_ref() else {
                return false;
            };
            let _ = direct3d_12::Device::create_query_heap(
                this,
                &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_query_heap_desc(
                    &[u8::MIN; 12],
                ),
                &Default::default(),
            );
            true
        });
    }
    // ID3D12Fence::GetCompletedValue on Fence
    if wanted(ID_3D12_FENCE) {
        driven.attempt(|| {
            let Some(this) = o.k55.as_ref() else {
                return false;
            };
            let _ = direct3d_12::Fence::get_completed_value(this);
            true
        });
    }
    // ID3D12Fence::SetEventOnCompletion on Fence
    if wanted(ID_3D12_FENCE) {
        driven.attempt(|| {
            let Some(this) = o.k55.as_ref() else {
                return false;
            };
            let _ = direct3d_12::Fence::set_event_on_completion(
                this,
                Default::default(),
                Default::default(),
            );
            true
        });
    }
    // ID3D12GraphicsCommandList::Close on GraphicsCommandList
    if wanted(ID_3D12_GRAPHICS_COMMAND_LIST) {
        driven.attempt(|| {
            let Some(this) = o.k56.as_ref() else {
                return false;
            };
            let _ = direct3d_12::GraphicsCommandList::close(this);
            true
        });
    }
    // ID3D12GraphicsCommandList::Reset on GraphicsCommandList
    if wanted(ID_3D12_GRAPHICS_COMMAND_LIST) {
        driven.attempt(|| {
            let Some(this) = o.k56.as_ref() else {
                return false;
            };
            let Some(v1) = o.k51.as_ref() else {
                return false;
            };
            let _ = direct3d_12::GraphicsCommandList::reset(this, v1, None);
            true
        });
    }
    // ID3D12GraphicsCommandList::DrawInstanced on GraphicsCommandList
    if wanted(ID_3D12_GRAPHICS_COMMAND_LIST) {
        driven.attempt(|| {
            let Some(this) = o.k56.as_ref() else {
                return false;
            };
            let _ = direct3d_12::GraphicsCommandList::draw_instanced(
                this,
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
            );
            true
        });
    }
    // ID3D12GraphicsCommandList::DrawIndexedInstanced on GraphicsCommandList
    if wanted(ID_3D12_GRAPHICS_COMMAND_LIST) {
        driven.attempt(|| {
            let Some(this) = o.k56.as_ref() else {
                return false;
            };
            let _ = direct3d_12::GraphicsCommandList::draw_indexed_instanced(
                this,
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
            );
            true
        });
    }
    // ID3D12GraphicsCommandList::Dispatch on GraphicsCommandList
    if wanted(ID_3D12_GRAPHICS_COMMAND_LIST) {
        driven.attempt(|| {
            let Some(this) = o.k56.as_ref() else {
                return false;
            };
            let _ = direct3d_12::GraphicsCommandList::dispatch(
                this,
                Default::default(),
                Default::default(),
                Default::default(),
            );
            true
        });
    }
    // ID3D12GraphicsCommandList::CopyBufferRegion on GraphicsCommandList
    if wanted(ID_3D12_GRAPHICS_COMMAND_LIST) {
        driven.attempt(|| {
            let Some(this) = o.k56.as_ref() else {
                return false;
            };
            let Some(v1) = o.k59.as_ref() else {
                return false;
            };
            let Some(v2) = o.k59.as_ref() else {
                return false;
            };
            let _ = direct3d_12::GraphicsCommandList::copy_buffer_region(
                this,
                v1,
                Default::default(),
                v2,
                Default::default(),
                Default::default(),
            );
            true
        });
    }
    // ID3D12GraphicsCommandList::IASetPrimitiveTopology on GraphicsCommandList
    if wanted(ID_3D12_GRAPHICS_COMMAND_LIST) {
        driven.attempt(|| {
            let Some(this) = o.k56.as_ref() else {
                return false;
            };
            let _ = direct3d_12::GraphicsCommandList::ia_set_primitive_topology(
                this,
                Default::default(),
            );
            true
        });
    }
    // ID3D12GraphicsCommandList::SetPipelineState on GraphicsCommandList
    if wanted(ID_3D12_GRAPHICS_COMMAND_LIST) {
        driven.attempt(|| {
            let Some(this) = o.k56.as_ref() else {
                return false;
            };
            let Some(v1) = o.k57.as_ref() else {
                return false;
            };
            let _ = direct3d_12::GraphicsCommandList::set_pipeline_state(this, v1);
            true
        });
    }
    // ID3D12GraphicsCommandList::SetDescriptorHeaps on GraphicsCommandList
    if wanted(ID_3D12_GRAPHICS_COMMAND_LIST) {
        driven.attempt(|| {
            let Some(this) = o.k56.as_ref() else {
                return false;
            };
            let _ = direct3d_12::GraphicsCommandList::set_descriptor_heaps(this, &[]);
            true
        });
    }
    // ID3D12GraphicsCommandList::SetComputeRootSignature on GraphicsCommandList
    if wanted(ID_3D12_GRAPHICS_COMMAND_LIST) {
        driven.attempt(|| {
            let Some(this) = o.k56.as_ref() else {
                return false;
            };
            let _ = direct3d_12::GraphicsCommandList::set_compute_root_signature(this, None);
            true
        });
    }
    // ID3D12GraphicsCommandList::SetGraphicsRootSignature on GraphicsCommandList
    if wanted(ID_3D12_GRAPHICS_COMMAND_LIST) {
        driven.attempt(|| {
            let Some(this) = o.k56.as_ref() else {
                return false;
            };
            let _ = direct3d_12::GraphicsCommandList::set_graphics_root_signature(this, None);
            true
        });
    }
    // ID3D12GraphicsCommandList::BeginQuery on GraphicsCommandList
    if wanted(ID_3D12_GRAPHICS_COMMAND_LIST) {
        driven.attempt(|| {
            let Some(this) = o.k56.as_ref() else {
                return false;
            };
            let Some(v1) = o.k58.as_ref() else {
                return false;
            };
            let _ = direct3d_12::GraphicsCommandList::begin_query(
                this,
                v1,
                Default::default(),
                Default::default(),
            );
            true
        });
    }
    // ID3D12GraphicsCommandList::EndQuery on GraphicsCommandList
    if wanted(ID_3D12_GRAPHICS_COMMAND_LIST) {
        driven.attempt(|| {
            let Some(this) = o.k56.as_ref() else {
                return false;
            };
            let Some(v1) = o.k58.as_ref() else {
                return false;
            };
            let _ = direct3d_12::GraphicsCommandList::end_query(
                this,
                v1,
                Default::default(),
                Default::default(),
            );
            true
        });
    }
    // ID3D12GraphicsCommandList::ResolveQueryData on GraphicsCommandList
    if wanted(ID_3D12_GRAPHICS_COMMAND_LIST) {
        driven.attempt(|| {
            let Some(this) = o.k56.as_ref() else {
                return false;
            };
            let Some(v1) = o.k58.as_ref() else {
                return false;
            };
            let Some(v2) = o.k59.as_ref() else {
                return false;
            };
            let _ = direct3d_12::GraphicsCommandList::resolve_query_data(
                this,
                v1,
                Default::default(),
                Default::default(),
                Default::default(),
                v2,
                Default::default(),
            );
            true
        });
    }
    // ID3D12Resource::GetDesc on Resource
    if wanted(ID_3D12_RESOURCE) {
        driven.attempt(|| {
            let Some(this) = o.k59.as_ref() else {
                return false;
            };
            let _ = direct3d_12::Resource::get_desc(this);
            true
        });
    }
    // D3D12CreateDevice
    if wanted(D_3D12_CREATE_DEVICE) {
        driven.attempt(|| {
            let _ = direct3d_12::Direct3D12::create_device(
                &o.api,
                None,
                Default::default(),
                &Default::default(),
            );
            true
        });
    }
    driven
}

/// The interface (or export) `D3D12CreateDevice`, by its name in the header.
const D_3D12_CREATE_DEVICE: &str = "D3D12CreateDevice";
/// The interface (or export) `ID3D12CommandAllocator`, by its name in the header.
const ID_3D12_COMMAND_ALLOCATOR: &str = "ID3D12CommandAllocator";
/// The interface (or export) `ID3D12CommandQueue`, by its name in the header.
const ID_3D12_COMMAND_QUEUE: &str = "ID3D12CommandQueue";
/// The interface (or export) `ID3D12DescriptorHeap`, by its name in the header.
const ID_3D12_DESCRIPTOR_HEAP: &str = "ID3D12DescriptorHeap";
/// The interface (or export) `ID3D12Device`, by its name in the header.
const ID_3D12_DEVICE: &str = "ID3D12Device";
/// The interface (or export) `ID3D12Fence`, by its name in the header.
const ID_3D12_FENCE: &str = "ID3D12Fence";
/// The interface (or export) `ID3D12GraphicsCommandList`, by its name in the header.
const ID_3D12_GRAPHICS_COMMAND_LIST: &str = "ID3D12GraphicsCommandList";
/// The interface (or export) `ID3D12Resource`, by its name in the header.
const ID_3D12_RESOURCE: &str = "ID3D12Resource";
/// The interface (or export) `ID3DBlob`, by its name in the header.
const ID_3D_BLOB: &str = "ID3DBlob";
/// The interface (or export) `ID3DDestructionNotifier`, by its name in the header.
const ID_3D_DESTRUCTION_NOTIFIER: &str = "ID3DDestructionNotifier";
/// Each effects judgment of the APIs in reach (src/api.rs `effects`): the interfaces (or exports) judged together, and
/// the judgment.
pub const EFFECTS: &[(&[&str], &str)] = &[
    (
        &["ID3DBlob"],
        "a block of bytes: its address (a kind lent until Release, refused) and its size",
    ),
    (
        &["ID3DDestructionNotifier"],
        "registers callbacks, whose function pointer is a kind the system trusts (refused by default), and unregisters them by a number",
    ),
    (
        &["D3D12CreateDevice"],
        "makes a device on an adapter, or answers whether it could; the adapter is an object, never a module",
    ),
    (
        &["ID3D12Device"],
        "makes objects, writes descriptors and answers questions of layout; a descriptor handle and a GPU address are the kinds the glue judges by their own rows",
    ),
    (
        &["ID3D12CommandAllocator"],
        "resets the memory of the lists it backs, which the GPU owns",
    ),
    (
        &["ID3D12CommandQueue"],
        "runs closed lists on the GPU and signals fences: work of the GPU on its own memory",
    ),
    (
        &["ID3D12DescriptorHeap"],
        "describes itself and answers its handles' starts",
    ),
    (
        &["ID3D12Fence"],
        "a fence's value and an event it signals (a handle the kernel checks)",
    ),
    (
        &["ID3D12GraphicsCommandList"],
        "records work for the GPU, run only when a queue executes the list: binds, draws, copies and barriers on the GPU's memory",
    ),
    (
        &["ID3D12Resource"],
        "describes itself and answers its GPU address; Map and Unmap lend memory until Unmap, a kind refused with no exception line",
    ),
];

/// The methods the rows keep from the system (never called): none reaches an implementation.
pub const NEVER_REACHED: &[&str] = &[];

/// ID3DBlob::GetBufferSize on Blob, of random arguments.
fn fuzz_0(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k0.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ =
        <direct3d_common_direct::SystemBlob<Api> as direct3d_common::Blob>::get_buffer_size(this);
    true
}

/// ID3DDestructionNotifier::UnregisterDestructionCallback on DestructionNotifier, of random arguments.
fn fuzz_1(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k1.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <direct3d_common_direct::SystemDestructionNotifier<Api> as direct3d_common::DestructionNotifier>::unregister_destruction_callback(this, r.value::<u32>());
    true
}

/// ID3D12CommandAllocator::Reset on CommandAllocator, of random arguments.
fn fuzz_2(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k51.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ =
        <direct3d_12_direct::SystemCommandAllocator<Api> as direct3d_12::CommandAllocator>::reset(
            this,
        );
    true
}

/// ID3D12CommandQueue::Signal on CommandQueue, of random arguments.
fn fuzz_3(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k52.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let Some(v1) = o.k55.as_ref() else {
        return false;
    };
    let _ = <direct3d_12_direct::SystemCommandQueue<Api> as direct3d_12::CommandQueue>::signal(
        this,
        v1,
        r.value::<u64>(),
    );
    true
}

/// ID3D12DescriptorHeap::GetDesc on DescriptorHeap, of random arguments.
fn fuzz_4(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k53.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ =
        <direct3d_12_direct::SystemDescriptorHeap<Api> as direct3d_12::DescriptorHeap>::get_desc(
            this,
        );
    true
}

/// ID3D12Device::CreateCommandQueue on Device, of random arguments.
fn fuzz_5(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k54.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <direct3d_12_direct::SystemDevice<Api> as direct3d_12::Device>::create_command_queue(
        this,
        &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_command_queue_desc(
            &r.bytes(16),
        ),
        &Default::default(),
    );
    true
}

/// ID3D12Device::CreateCommandAllocator on Device, of random arguments.
fn fuzz_6(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k54.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ =
        <direct3d_12_direct::SystemDevice<Api> as direct3d_12::Device>::create_command_allocator(
            this,
            direct3d_12::enumerations::CommandListType(r.value()),
            &Default::default(),
        );
    true
}

/// ID3D12Device::CreateCommandList on Device, of random arguments.
fn fuzz_7(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k54.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let Some(v1) = o.k51.as_ref() else {
        return false;
    };
    let _ = <direct3d_12_direct::SystemDevice<Api> as direct3d_12::Device>::create_command_list(
        this,
        r.value::<u32>(),
        direct3d_12::enumerations::CommandListType(r.value()),
        v1,
        o.k57.as_ref().filter(|_| r.coin()),
        &Default::default(),
    );
    true
}

/// ID3D12Device::CreateDescriptorHeap on Device, of random arguments.
fn fuzz_8(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k54.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <direct3d_12_direct::SystemDevice<Api> as direct3d_12::Device>::create_descriptor_heap(
        this,
        &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_descriptor_heap_desc(
            &r.bytes(16),
        ),
        &Default::default(),
    );
    true
}

/// ID3D12Device::GetDescriptorHandleIncrementSize on Device, of random arguments.
fn fuzz_9(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k54.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <direct3d_12_direct::SystemDevice<Api> as direct3d_12::Device>::get_descriptor_handle_increment_size(this, direct3d_12::enumerations::DescriptorHeapType(r.value()));
    true
}

/// ID3D12Device::CreateRootSignature on Device, of random arguments.
fn fuzz_10(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k54.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let s1: Vec<_> = (0..count).map(|_| r.value::<u8>()).collect();
    let _ = <direct3d_12_direct::SystemDevice<Api> as direct3d_12::Device>::create_root_signature(
        this,
        r.value::<u32>(),
        s1.as_slice(),
        &Default::default(),
    );
    true
}

/// ID3D12Device::CreateCommittedResource on Device, of random arguments.
fn fuzz_11(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k54.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let v1 = if r.coin() {
        Some(
            &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_clear_value(
                &r.bytes(20),
            ),
        )
    } else {
        None
    };
    let _ =
        <direct3d_12_direct::SystemDevice<Api> as direct3d_12::Device>::create_committed_resource(
            this,
            &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_heap_properties(
                &r.bytes(20),
            ),
            direct3d_12::enumerations::HeapFlags(r.value()),
            &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_resource_desc(
                &r.bytes(56),
            ),
            direct3d_12::enumerations::ResourceStates(r.value()),
            v1,
            &Default::default(),
        );
    true
}

/// ID3D12Device::CreateFence on Device, of random arguments.
fn fuzz_12(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k54.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <direct3d_12_direct::SystemDevice<Api> as direct3d_12::Device>::create_fence(
        this,
        r.value::<u64>(),
        direct3d_12::enumerations::FenceFlags(r.value()),
        &Default::default(),
    );
    true
}

/// ID3D12Device::GetCopyableFootprints on Device, of random arguments.
fn fuzz_13(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k54.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <direct3d_12_direct::SystemDevice<Api> as direct3d_12::Device>::get_copyable_footprints(
        this,
        &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_resource_desc(
            &r.bytes(56),
        ),
        r.value::<u32>(),
        r.value::<u32>(),
        r.value::<u64>(),
    );
    true
}

/// ID3D12Device::CreateQueryHeap on Device, of random arguments.
fn fuzz_14(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k54.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <direct3d_12_direct::SystemDevice<Api> as direct3d_12::Device>::create_query_heap(
        this,
        &data_direct3d_12::decode_windows_win32_graphics_direct_3d12_d_3d12_query_heap_desc(
            &r.bytes(12),
        ),
        &Default::default(),
    );
    true
}

/// ID3D12Fence::GetCompletedValue on Fence, of random arguments.
fn fuzz_15(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k55.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <direct3d_12_direct::SystemFence<Api> as direct3d_12::Fence>::get_completed_value(this);
    true
}

/// ID3D12Fence::SetEventOnCompletion on Fence, of random arguments.
fn fuzz_16(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k55.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <direct3d_12_direct::SystemFence<Api> as direct3d_12::Fence>::set_event_on_completion(
        this,
        r.value::<u64>(),
        Default::default(),
    );
    true
}

/// ID3D12GraphicsCommandList::Close on GraphicsCommandList, of random arguments.
fn fuzz_17(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k56.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <direct3d_12_direct::SystemGraphicsCommandList<Api> as direct3d_12::GraphicsCommandList>::close(this);
    true
}

/// ID3D12GraphicsCommandList::Reset on GraphicsCommandList, of random arguments.
fn fuzz_18(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k56.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let Some(v1) = o.k51.as_ref() else {
        return false;
    };
    let _ = <direct3d_12_direct::SystemGraphicsCommandList<Api> as direct3d_12::GraphicsCommandList>::reset(this, v1, o.k57.as_ref().filter(|_| r.coin()));
    true
}

/// ID3D12GraphicsCommandList::DrawInstanced on GraphicsCommandList, of random arguments.
fn fuzz_19(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k56.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <direct3d_12_direct::SystemGraphicsCommandList<Api> as direct3d_12::GraphicsCommandList>::draw_instanced(this, r.value::<u32>(), r.value::<u32>(), r.value::<u32>(), r.value::<u32>());
    true
}

/// ID3D12GraphicsCommandList::DrawIndexedInstanced on GraphicsCommandList, of random arguments.
fn fuzz_20(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k56.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <direct3d_12_direct::SystemGraphicsCommandList<Api> as direct3d_12::GraphicsCommandList>::draw_indexed_instanced(this, r.value::<u32>(), r.value::<u32>(), r.value::<u32>(), r.value::<i32>(), r.value::<u32>());
    true
}

/// ID3D12GraphicsCommandList::Dispatch on GraphicsCommandList, of random arguments.
fn fuzz_21(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k56.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <direct3d_12_direct::SystemGraphicsCommandList<Api> as direct3d_12::GraphicsCommandList>::dispatch(this, r.value::<u32>(), r.value::<u32>(), r.value::<u32>());
    true
}

/// ID3D12GraphicsCommandList::CopyBufferRegion on GraphicsCommandList, of random arguments.
fn fuzz_22(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k56.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let Some(v1) = o.k59.as_ref() else {
        return false;
    };
    let Some(v2) = o.k59.as_ref() else {
        return false;
    };
    let _ = <direct3d_12_direct::SystemGraphicsCommandList<Api> as direct3d_12::GraphicsCommandList>::copy_buffer_region(this, v1, r.value::<u64>(), v2, r.value::<u64>(), r.value::<u64>());
    true
}

/// ID3D12GraphicsCommandList::IASetPrimitiveTopology on GraphicsCommandList, of random arguments.
fn fuzz_23(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k56.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <direct3d_12_direct::SystemGraphicsCommandList<Api> as direct3d_12::GraphicsCommandList>::ia_set_primitive_topology(this, direct3d_common::enumerations::PrimitiveTopology(r.value()));
    true
}

/// ID3D12GraphicsCommandList::SetPipelineState on GraphicsCommandList, of random arguments.
fn fuzz_24(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k56.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let Some(v1) = o.k57.as_ref() else {
        return false;
    };
    let _ = <direct3d_12_direct::SystemGraphicsCommandList<Api> as direct3d_12::GraphicsCommandList>::set_pipeline_state(this, v1);
    true
}

/// ID3D12GraphicsCommandList::SetDescriptorHeaps on GraphicsCommandList, of random arguments.
fn fuzz_25(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k56.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let s1: Vec<_> = (0..count)
        .map(|_| o.k53.as_ref().filter(|_| r.coin()))
        .collect();
    let _ = <direct3d_12_direct::SystemGraphicsCommandList<Api> as direct3d_12::GraphicsCommandList>::set_descriptor_heaps(this, s1.as_slice());
    true
}

/// ID3D12GraphicsCommandList::SetComputeRootSignature on GraphicsCommandList, of random arguments.
fn fuzz_26(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k56.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <direct3d_12_direct::SystemGraphicsCommandList<Api> as direct3d_12::GraphicsCommandList>::set_compute_root_signature(this, o.k60.as_ref().filter(|_| r.coin()));
    true
}

/// ID3D12GraphicsCommandList::SetGraphicsRootSignature on GraphicsCommandList, of random arguments.
fn fuzz_27(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k56.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <direct3d_12_direct::SystemGraphicsCommandList<Api> as direct3d_12::GraphicsCommandList>::set_graphics_root_signature(this, o.k60.as_ref().filter(|_| r.coin()));
    true
}

/// ID3D12GraphicsCommandList::BeginQuery on GraphicsCommandList, of random arguments.
fn fuzz_28(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k56.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let Some(v1) = o.k58.as_ref() else {
        return false;
    };
    let _ = <direct3d_12_direct::SystemGraphicsCommandList<Api> as direct3d_12::GraphicsCommandList>::begin_query(this, v1, direct3d_12::enumerations::QueryType(r.value()), r.value::<u32>());
    true
}

/// ID3D12GraphicsCommandList::EndQuery on GraphicsCommandList, of random arguments.
fn fuzz_29(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k56.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let Some(v1) = o.k58.as_ref() else {
        return false;
    };
    let _ = <direct3d_12_direct::SystemGraphicsCommandList<Api> as direct3d_12::GraphicsCommandList>::end_query(this, v1, direct3d_12::enumerations::QueryType(r.value()), r.value::<u32>());
    true
}

/// ID3D12GraphicsCommandList::ResolveQueryData on GraphicsCommandList, of random arguments.
fn fuzz_30(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k56.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let Some(v1) = o.k58.as_ref() else {
        return false;
    };
    let Some(v2) = o.k59.as_ref() else {
        return false;
    };
    let _ = <direct3d_12_direct::SystemGraphicsCommandList<Api> as direct3d_12::GraphicsCommandList>::resolve_query_data(this, v1, direct3d_12::enumerations::QueryType(r.value()), r.value::<u32>(), r.value::<u32>(), v2, r.value::<u64>());
    true
}

/// ID3D12Resource::GetDesc on Resource, of random arguments.
fn fuzz_31(o: &Objects, r: &mut Rng) -> bool {
    let Some(this) = o.k59.as_ref() else {
        return false;
    };
    let count = r.below(SLICE_LENGTHS);
    let _ = <direct3d_12_direct::SystemResource<Api> as direct3d_12::Resource>::get_desc(this);
    true
}

/// D3D12CreateDevice, of random arguments.
fn fuzz_32(o: &Objects, r: &mut Rng) -> bool {
    let count = r.below(SLICE_LENGTHS);
    let _ = <Api as direct3d_12::Direct3D12>::create_device(
        &o.api,
        None,
        direct3d_common::enumerations::FeatureLevel(r.value()),
        &Default::default(),
    );
    true
}

/// Drops each object but those the library's exports made, one time in `one_in`.
fn drop_some(o: &mut Objects, r: &mut Rng, one_in: usize) {
    if r.below(one_in) == 0 {
        o.k0 = None;
    }
    if r.below(one_in) == 0 {
        o.k1 = None;
    }
}

/// Every method's fuzzer, picked from at random.
const FUZZERS: [fn(&Objects, &mut Rng) -> bool; 33] = [
    fuzz_0, fuzz_1, fuzz_2, fuzz_3, fuzz_4, fuzz_5, fuzz_6, fuzz_7, fuzz_8, fuzz_9, fuzz_10,
    fuzz_11, fuzz_12, fuzz_13, fuzz_14, fuzz_15, fuzz_16, fuzz_17, fuzz_18, fuzz_19, fuzz_20,
    fuzz_21, fuzz_22, fuzz_23, fuzz_24, fuzz_25, fuzz_26, fuzz_27, fuzz_28, fuzz_29, fuzz_30,
    fuzz_31, fuzz_32,
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
