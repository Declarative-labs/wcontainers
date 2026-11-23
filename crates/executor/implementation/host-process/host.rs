//! The process-wide part of form P: the start-up probe, one zygote, forked anew when it ends, and the window cache.
//!
//! A private stand-in for the process-wide host the owner is asked for (plan 8.5); his ruling changes this one place.

use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

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
}

impl HostProcessHost {
    /// Probes the host and forks the first zygote from the calling thread.
    pub fn new() -> Result<Self, Unavailable> {
        let probe = HostProbe::new()?;
        let zygote = Zygote::start(&probe)?;
        let windows = WindowCache::new(WINDOWS).map_err(Unavailable::Host)?;
        Ok(Self {
            probe,
            zygote: Mutex::new(Ok(zygote)),
            zygote_ends: AtomicU64::new(0),
            windows: Mutex::new(windows),
        })
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
