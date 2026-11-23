//! The process-wide part of form P: the start-up probe, one zygote, forked anew when it ends, the window cache and the
//! table of CPUs its runners hold.
//!
//! A private stand-in for the process-wide host the owner is asked for (plan 8.5); his ruling changes this one place.

use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use rustix::io::Errno;

use crate::probe::{HostProbe, Unavailable};
use crate::stub::Stub;
use crate::windows::{WINDOWS, WindowCache};
use crate::zygote::{Ended, Zygote};

/// One per embedder process; every P executor of the process forks its stubs through it.
pub struct HostProcessHost {
    probe: HostProbe,
    /// The zygote, or why P became unavailable for good when a zygote could not be forked anew.
    zygote: Mutex<Result<Zygote, Unavailable>>,
    zygote_ends: AtomicU64,
    /// Every view of every stub's memory that the embedder's process has.
    windows: Mutex<WindowCache>,
    cpus: Mutex<CpuTable>,
}

/// How long a CPU found contended is passed over.
const CONTENDED_FOR: Duration = Duration::from_secs(1);

/// The CPUs the process may use: for each, how many runners hold it and when it was last found contended.
struct CpuTable {
    allowed: Vec<usize>,
    held: Vec<u32>,
    contended: Vec<Option<Instant>>,
}

impl CpuTable {
    fn new() -> Result<Self, Errno> {
        let set = rustix::thread::sched_getaffinity(None)?;
        let allowed: Vec<usize> = (0..rustix::thread::CpuSet::MAX_CPU)
            .filter(|cpu| set.is_set(*cpu))
            .collect();
        let size = allowed.last().map_or(0, |last| last + 1);
        Ok(Self {
            allowed,
            held: vec![0; size],
            contended: vec![None; size],
        })
    }
}

impl HostProcessHost {
    /// Probes the host and forks the first zygote from the calling thread.
    pub fn new() -> Result<Self, Unavailable> {
        let probe = HostProbe::new()?;
        let zygote = Zygote::start(&probe)?;
        let windows = WindowCache::new(WINDOWS).map_err(Unavailable::Host)?;
        let cpus = CpuTable::new().map_err(Unavailable::Host)?;
        Ok(Self {
            probe,
            zygote: Mutex::new(Ok(zygote)),
            zygote_ends: AtomicU64::new(0),
            windows: Mutex::new(windows),
            cpus: Mutex::new(cpus),
        })
    }

    /// Holds a CPU for a runner: the one held by the fewest runners and not found contended in the last second (the
    /// lowest number on a tie), or, when every one was, the one held by the fewest.
    pub fn take_cpu(&self) -> usize {
        let mut table = self
            .cpus
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let now = Instant::now();
        let fewest = |fresh_only: bool| {
            table
                .allowed
                .iter()
                .copied()
                .filter(|cpu| {
                    !fresh_only
                        || table.contended[*cpu]
                            .is_none_or(|at| now.duration_since(at) >= CONTENDED_FOR)
                })
                .min_by_key(|cpu| (table.held[*cpu], *cpu))
        };
        let cpu = fewest(true)
            .or_else(|| fewest(false))
            .expect("a process may use at least one CPU");
        table.held[cpu] += 1;
        cpu
    }

    /// Lets a runner's CPU go.
    pub fn release_cpu(&self, cpu: usize) {
        let mut table = self
            .cpus
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        table.held[cpu] = table.held[cpu].saturating_sub(1);
    }

    /// Marks a CPU found contended now.
    pub fn mark_contended(&self, cpu: usize) {
        let mut table = self
            .cpus
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        table.contended[cpu] = Some(Instant::now());
    }

    /// Reads the CPUs the process may use again, after a pin was refused; the counts of CPUs still allowed stay.
    pub fn reread_cpus(&self) -> Result<(), Errno> {
        let fresh = CpuTable::new()?;
        let mut table = self
            .cpus
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let size = fresh.held.len().max(table.held.len());
        table.held.resize(size, 0);
        table.contended.resize(size, None);
        table.allowed = fresh.allowed;
        Ok(())
    }

    /// The window cache, for one executor call's copies.
    pub(crate) fn windows(&self) -> std::sync::MutexGuard<'_, WindowCache> {
        self.windows
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// How many times the zygote ended ("zygote ended"), taking every stub it had forked with it.
    pub fn zygote_ends(&self) -> u64 {
        self.zygote_ends.load(Ordering::Relaxed)
    }

    /// Forks a stub; a zygote that has ended is reaped, counted, warned, and forked anew first.
    pub fn fork_stub(&self) -> Result<Stub, Unavailable> {
        let mut held = self
            .zygote
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let ended = match &*held {
            Err(why) => return Err(*why),
            Ok(zygote) => zygote.ping().is_err(),
        };
        if ended {
            let new = Zygote::start(&self.probe);
            if let Ok(old) = core::mem::replace(&mut *held, new) {
                let end = old.end();
                let times = self.zygote_ends.fetch_add(1, Ordering::Relaxed) + 1;
                eprintln!(
                    "wcontainers warning: zygote ended ({end:?}), every guest process it held ended as killed, {times} so far"
                );
            }
        }
        match &*held {
            Ok(zygote) => zygote.fork_stub(),
            Err(why) => Err(*why),
        }
    }

    /// Kills a stub; one whose zygote has ended died with it.
    pub fn kill(&self, stub: &Stub) -> Result<(), Errno> {
        match &*self.lock() {
            Ok(zygote) if zygote.forked(stub) => zygote.kill(stub.process()),
            _ => Ok(()),
        }
    }

    /// Waits for a stub's end and reaps it; one whose zygote has ended died with it, killed.
    pub fn reap(&self, stub: &Stub) -> Result<Ended, Errno> {
        match &*self.lock() {
            Ok(zygote) if zygote.forked(stub) => zygote.reap(stub.process()),
            _ => Ok(Ended::Killed(
                rustix::process::Signal::KILL.as_raw().unsigned_abs(),
            )),
        }
    }

    /// The zygote's process id, while there is one.
    pub fn zygote_process(&self) -> Option<rustix::process::Pid> {
        self.lock().as_ref().ok().map(Zygote::process)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Result<Zygote, Unavailable>> {
        self.zygote
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}
