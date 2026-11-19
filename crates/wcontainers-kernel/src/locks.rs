//! Record locks (`fcntl`'s F_SETLK family), as Linux's `fs/locks.c` keeps them on an inode: one list
//! of byte ranges, each held by an owner; each owner's ranges lie together in the list, sorted, with
//! neighbouring ranges of one kind joined, and an owner's first range goes at the end.

use alloc::rc::Weak;
use alloc::vec::Vec;
use core::cell::RefCell;

use crate::file::OpenFile;

/// Linux's `OFFSET_MAX`: where a lock to the end of the file ends.
pub(crate) const OFFSET_MAX: u64 = i64::MAX as u64;

/// Who holds a record lock (Linux's `fl_owner`).
#[derive(Clone)]
pub(crate) enum LockOwner {
    /// A descriptor table, for F_SETLK's locks: let go when any of its descriptors of the file closes.
    Table(u64),
    /// An open file, for F_OFD_SETLK's locks: let go when the file's last reference goes.
    File(Weak<RefCell<OpenFile>>),
}

impl LockOwner {
    /// Linux's `posix_same_owner`.
    fn same(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Table(one), Self::Table(other)) => one == other,
            (Self::File(one), Self::File(other)) => one.ptr_eq(other),
            _ => false,
        }
    }

    fn alive(&self) -> bool {
        match self {
            Self::Table(_) => true,
            Self::File(file) => file.strong_count() > 0,
        }
    }

    pub(crate) fn is_file(&self) -> bool {
        matches!(self, Self::File(_))
    }
}

/// One held range, or a request for one.
#[derive(Clone)]
pub(crate) struct RecordLock {
    pub owner: LockOwner,
    /// The process that took it (`flc_pid`), and the open file it was taken through (`flc_file`).
    pub pid: i32,
    pub file: Weak<RefCell<OpenFile>>,
    pub write: bool,
    pub start: u64,
    /// The last byte, inclusive.
    pub end: u64,
}

impl RecordLock {
    fn overlaps(&self, other: &Self) -> bool {
        self.start <= other.end && other.start <= self.end
    }

    /// Linux's `posix_locks_conflict`.
    fn conflicts(&self, held: &Self) -> bool {
        !self.owner.same(&held.owner) && self.overlaps(held) && (self.write || held.write)
    }
}

/// Linux's `posix_test_lock`: the first held lock a request would meet; for an unlock, the first of
/// the owner's own that overlaps it.
pub(crate) fn test(
    locks: &mut Vec<RecordLock>,
    request: &RecordLock,
    unlock: bool,
) -> Option<RecordLock> {
    locks.retain(|held| held.owner.alive());
    locks
        .iter()
        .find(|held| {
            if unlock {
                held.owner.same(&request.owner) && held.overlaps(request)
            } else {
                request.conflicts(held)
            }
        })
        .cloned()
}

/// Linux's `posix_lock_inode`: takes the range for its owner, or lets it go with `unlock`. Another
/// owner's lock in the way is the error. Says whether any of the owner's locks was cut, joined or
/// let go, which wakes the requests it blocked.
pub(crate) fn set(
    locks: &mut Vec<RecordLock>,
    request: RecordLock,
    unlock: bool,
) -> Result<bool, RecordLock> {
    locks.retain(|held| held.owner.alive());
    if !unlock && let Some(held) = locks.iter().find(|held| request.conflicts(held)) {
        return Err(held.clone());
    }
    let at = locks
        .iter()
        .position(|held| held.owner.same(&request.owner))
        .unwrap_or(locks.len());
    let own: Vec<RecordLock> = locks
        .extract_if(.., |held| held.owner.same(&request.owner))
        .collect();
    // The owner's ranges with the request's cut out, then the request (marked as it).
    let mut kept: Vec<(RecordLock, bool)> = Vec::with_capacity(own.len() + 2);
    let mut changed = false;
    for held in own {
        if held.end < request.start || held.start > request.end {
            // A neighbour of the same kind is joined to the request.
            changed |= !unlock
                && held.write == request.write
                && (held.end.checked_add(1) == Some(request.start)
                    || request.end.checked_add(1) == Some(held.start));
            kept.push((held, false));
            continue;
        }
        changed = true;
        if held.start < request.start {
            kept.push((
                RecordLock {
                    end: request.start - 1,
                    ..held.clone()
                },
                false,
            ));
        }
        if held.end > request.end {
            kept.push((
                RecordLock {
                    start: request.end + 1,
                    ..held
                },
                false,
            ));
        }
    }
    if !unlock {
        kept.push((request, true));
    }
    kept.sort_by_key(|(held, _)| held.start);
    let mut joined: Vec<(RecordLock, bool)> = Vec::with_capacity(kept.len());
    for (held, is_request) in kept {
        if let Some((last, last_is_request)) = joined.last_mut()
            && last.write == held.write
            && last.end.checked_add(1) == Some(held.start)
        {
            // A request joining a held lock grows the held one, which keeps its taker and file.
            if *last_is_request && !is_request {
                *last = RecordLock {
                    start: last.start,
                    ..held
                };
                *last_is_request = false;
            } else {
                last.end = held.end;
            }
            continue;
        }
        joined.push((held, is_request));
    }
    locks.splice(at..at, joined.into_iter().map(|(held, _)| held));
    Ok(changed)
}

/// Lets go of every lock a descriptor table holds on a file (`locks_remove_posix`); says whether it
/// held any.
pub(crate) fn release_table(locks: &mut Vec<RecordLock>, table: u64) -> bool {
    let before = locks.len();
    locks.retain(|held| !matches!(held.owner, LockOwner::Table(owner) if owner == table));
    locks.len() != before
}

/// Lets go of the locks of open files that are gone (`locks_remove_file`); says whether there were
/// any.
pub(crate) fn release_dead(locks: &mut Vec<RecordLock>) -> bool {
    let before = locks.len();
    locks.retain(|held| held.owner.alive());
    locks.len() != before
}
