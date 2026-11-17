//! The file system that lives outside the kernel: every operation on it is the FUSE message Linux
//! would send, handed to the host through its one backend hook, and the answer comes back as an
//! event.
//!
//! A call that needs an answer asks and parks. When the answer comes, the call runs again from its
//! start, and each question it asks again is answered from the replies its earlier runs had, in
//! order, so a call sends each message once however often it runs. Nothing else is kept: names,
//! attributes and bytes are asked for every time (caching is the first speed-up after phase one).

use alloc::collections::BTreeMap;
use alloc::rc::{Rc, Weak};
use alloc::vec::Vec;
use core::cell::{Cell, RefCell};

use fuse_abi::linux::*;
use zerocopy::{FromBytes, Immutable, IntoBytes};

use super::{Content, FileSystem, Inode, Node, Place, Time};
use crate::Errno;
use crate::process::Tid;

/// The FUSE protocol version the kernel speaks (Linux 6.9's, which `fuse-abi` describes).
const MINOR: u32 = FUSE_KERNEL_MINOR_VERSION;

/// What INIT asks the backend for: Linux's own `fuse_send_init` flags, less the ones that let it
/// keep names, attributes or bytes (no cache in phase one) or hand locks to the backend (they are
/// kept here).
const INIT_FLAGS: u32 = FUSE_ASYNC_READ
    | FUSE_ATOMIC_O_TRUNC
    | FUSE_EXPORT_SUPPORT
    | FUSE_BIG_WRITES
    | FUSE_DONT_MASK
    | FUSE_AUTO_INVAL_DATA
    | FUSE_ASYNC_DIO
    | FUSE_DO_READDIRPLUS
    | FUSE_READDIRPLUS_AUTO
    | FUSE_PARALLEL_DIROPS
    | FUSE_ABORT_ERROR
    | FUSE_MAX_PAGES
    | FUSE_INIT_EXT;

/// The most a read or write asks for in one message until INIT says (Linux's default `max_pages`).
const DEFAULT_CHUNK: u32 = 32 * 4096;

/// What INIT's answer settled.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Settled {
    pub flags: u32,
    pub max_write: u32,
    pub max_read: u32,
}

/// A request a call made: its number, what it asked (opcode, node and body, hashed), and the
/// attribute version when it was sent (Linux's `fuse_get_attr_version`).
#[derive(Clone, Copy)]
struct Request {
    unique: u64,
    opcode: u32,
    nodeid: u64,
    asked: u64,
    version: u64,
}

/// An answer a call had, with the request it answers.
#[derive(Clone)]
struct Reply {
    request: Request,
    result: Result<Rc<[u8]>, Errno>,
}

/// One call's questions: the replies its runs have had, how many of them this run has used, and
/// the request it waits for.
#[derive(Clone, Default)]
struct Asked {
    replies: Vec<Reply>,
    used: usize,
    waiting: Option<Request>,
    /// The inodes its answers named and the handles they gave, held until the call ends, as
    /// Linux holds what a call has looked up and opened: dropped between runs, the backend would
    /// be told to forget or release them.
    nodes: Vec<Node>,
    handles: BTreeMap<usize, Rc<Handle>>,
}

/// Who asks, as the request header names them.
#[derive(Clone, Copy)]
pub(crate) struct Asker {
    pub tid: Tid,
    pub uid: u32,
    pub gid: u32,
    pub pid: u32,
}

/// A request the host has: the thread whose call waits for it (none when nothing does), its opcode,
/// and when it was handed over.
#[derive(Clone, Copy)]
struct Sent {
    tid: Option<Tid>,
    opcode: u32,
    nodeid: u64,
    at: u64,
}

/// A request made and not yet handed to the host.
pub(crate) struct Outgoing {
    pub unique: u64,
    pub opcode: u32,
    pub bytes: Vec<u8>,
    /// Whether the backend answers it.
    pub answered: bool,
}

/// What an answer from the backend did.
pub(crate) enum Answered {
    /// A waiting call has it; its thread runs the call again.
    Call(Tid, u32, u64),
    /// Nothing waits for it (its opcode and when it was sent, if it was).
    Nobody(Option<(u32, u64)>),
}

#[derive(Default)]
struct State {
    last: u64,
    asker: Option<Asker>,
    /// Counts each run of a call, for which attributes a run has already had.
    run: u64,
    /// Counts changes to attributes the kernel holds (Linux's `fc->attr_version`).
    attr_version: u64,
    /// The attribute version of the request whose answer was taken last in this run.
    taken: Option<u64>,
    /// Opcodes of questions a call asked again because its earlier answer was to another question.
    asked_again: Vec<u32>,
    calls: BTreeMap<Tid, Asked>,
    outbox: Vec<Outgoing>,
    sent: BTreeMap<u64, Sent>,
    /// Inodes the backend numbered, while the kernel holds them.
    nodes: BTreeMap<u64, Weak<RefCell<Inode>>>,
    /// Lookups the backend counted for each node, given back with FORGET when its inode goes.
    lookups: BTreeMap<u64, u64>,
    settled: Option<Settled>,
}

/// The connection to one backend, reachable from every inode on it.
pub(crate) struct Client {
    me: Weak<Client>,
    state: RefCell<State>,
    /// The file system its inodes are on.
    pub fs: FileSystem,
}

/// An inode of the backend: its node number there, and what keeps its place in the tree.
pub(crate) struct Remote {
    pub client: Rc<Client>,
    pub nodeid: u64,
    /// The run of a call whose reply last gave its attributes.
    fresh: Cell<u64>,
    /// The attribute version of its last change (Linux's `fi->attr_version`): an answer to a
    /// request sent before it is older than what the inode holds.
    version: Cell<u64>,
    /// The size, blocks and block size the last answer gave.
    pub size: Cell<u64>,
    pub blocks: Cell<u64>,
    pub block_size: Cell<u32>,
    /// While a mapping shows the file: the pages its mappings show.
    pub pages: Option<Pages>,
    /// The thread whose call took its writes' lock last (`io::lock_backend_writes`).
    pub writer: Cell<Option<Tid>>,
    /// The access and default access control lists the inode holds (`i_acl`, `i_default_acl`):
    /// not held, or the list a read gave, empty for none. A permission check by another than the
    /// owner holds none for the access list (`check_acl` through `fuse_get_inode_acl`).
    pub acls: RefCell<[Option<Vec<u8>>; 2]>,
}

/// A mapped backend file's first `block.length` bytes in a block of shared pages, as Linux's page
/// cache holds a mapped file: read from the backend when mapped, kept up to date by reads and
/// writes through the kernel, and written back while a shared mapping may write them.
pub(crate) struct Pages {
    pub block: super::Block,
    /// The open file its bytes are read and written back through (`fuse_write_file_get`).
    pub handle: Rc<Handle>,
    /// Whether a shared mapping that may write has shown it, so its bytes go back to the backend.
    pub written: bool,
}

impl Remote {
    /// The file's size changed here (`fuse_write_update_attr`, a truncation): answers to requests
    /// sent before it are older.
    pub fn set_size(&self, size: u64) {
        self.size.set(size);
        self.changed();
    }

    /// What the inode holds changed, or the backend said it did (`fuse_reverse_inval_inode`).
    pub fn changed(&self) {
        self.version.set(self.client.next_version());
    }

    /// Lets go of the access control lists held (`forget_all_cached_acls`).
    pub fn forget_acls(&self) {
        *self.acls.borrow_mut() = [None, None];
    }
}

/// The backend inode an inode is, a named pipe's too.
pub(crate) fn remote_of(inode: &Inode) -> Option<&Remote> {
    match &inode.content {
        Content::Backend(remote) => Some(remote),
        Content::Pipe(pipe) => pipe.backend.as_deref(),
        _ => None,
    }
}

impl Drop for Remote {
    /// The backend may forget the node once the kernel holds it no more (`fuse_evict_inode`).
    fn drop(&mut self) {
        if self.nodeid != FUSE_ROOT_ID {
            self.client.forget(self.nodeid);
        }
    }
}

/// An open file or directory on the backend: its handle there, given back with RELEASE when the
/// last descriptor and mapping of it go (`fuse_file_put`).
pub(crate) struct Handle {
    pub client: Rc<Client>,
    pub nodeid: u64,
    pub fh: u64,
    pub flags: u32,
    /// The open reply's `FOPEN_*` flags.
    pub open_flags: u32,
    pub directory: bool,
}

impl Handle {
    /// Whether a shared mapping of the file may be made (`fuse_file_mmap`): not of a file the
    /// backend opened for direct reads and writes, since INIT does not ask to map those
    /// (FUSE_DIRECT_IO_ALLOW_MMAP).
    pub fn may_map_shared(&self) -> bool {
        !self.direct_io()
    }

    /// Whether the backend opened the file for direct reads and writes (FOPEN_DIRECT_IO).
    pub fn direct_io(&self) -> bool {
        self.open_flags & FOPEN_DIRECT_IO != 0
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        let release = fuse_release_in {
            fh: self.fh,
            flags: self.flags,
            release_flags: 0,
            lock_owner: 0,
        };
        let opcode = if self.directory {
            fuse_opcode::FUSE_RELEASEDIR
        } else {
            fuse_opcode::FUSE_RELEASE
        };
        self.client
            .tell(opcode as u32, self.nodeid, &[release.as_bytes()]);
    }
}

fn bytes_of<T: IntoBytes + Immutable>(value: &T) -> &[u8] {
    value.as_bytes()
}

impl Client {
    /// A connection whose first message is INIT.
    pub fn new(fs: FileSystem) -> Rc<Self> {
        let client = Rc::new_cyclic(|me| Self {
            me: me.clone(),
            state: RefCell::default(),
            fs,
        });
        let init = fuse_init_in {
            major: FUSE_KERNEL_VERSION,
            minor: MINOR,
            max_readahead: 0,
            flags: INIT_FLAGS,
            flags2: 0,
            unused: [0; 11],
        };
        client.tell(fuse_opcode::FUSE_INIT as u32, 0, &[bytes_of(&init)]);
        client
    }

    /// What INIT settled, or Linux's defaults until it is answered.
    pub fn settled(&self) -> Settled {
        self.state.borrow().settled.unwrap_or(Settled {
            flags: 0,
            max_write: DEFAULT_CHUNK,
            max_read: DEFAULT_CHUNK,
        })
    }

    /// A run of `asker`'s call starts: its questions are answered from its replies so far.
    pub fn begin(&self, asker: Asker) {
        let mut state = self.state.borrow_mut();
        state.run += 1;
        state.asker = Some(asker);
        state.taken = None;
        if let Some(asked) = state.calls.get_mut(&asker.tid) {
            asked.used = 0;
        }
    }

    /// The run ended; with `done`, so did the call, and its replies go.
    pub fn end(&self, tid: Tid, done: bool) {
        let ended = {
            let mut state = self.state.borrow_mut();
            state.asker = None;
            if done { state.calls.remove(&tid) } else { None }
        };
        // What the call held goes once the connection is free to be told.
        drop(ended);
    }

    /// A thread ended: its call's replies go.
    pub fn drop_call(&self, tid: Tid) {
        let ended = self.state.borrow_mut().calls.remove(&tid);
        drop(ended);
    }

    /// The handle the answer just taken gives, made once however often the call runs.
    pub fn handle(&self, make: impl FnOnce() -> Handle) -> Rc<Handle> {
        let mut state = self.state.borrow_mut();
        let tid = state.asker.expect("a handle outside a system call").tid;
        let asked = state.calls.get_mut(&tid).expect("an answer was taken");
        let index = asked.used - 1;
        asked
            .handles
            .entry(index)
            .or_insert_with(|| Rc::new(make()))
            .clone()
    }

    /// Whether messages wait to be handed to the host.
    pub fn has_outgoing(&self) -> bool {
        !self.state.borrow().outbox.is_empty()
    }

    /// Whether `tid`'s call waits for an answer.
    pub fn waits(&self, tid: Tid) -> bool {
        self.state
            .borrow()
            .calls
            .get(&tid)
            .is_some_and(|asked| asked.waiting.is_some())
    }

    /// Asks the backend: the answer's body, the error it gave, or `PARKED` while it is awaited.
    /// An answer an earlier run had is taken only for the same question; a call that asks
    /// something else in its place (what it read changed while it waited) drops that answer and
    /// the ones after it, and asks afresh.
    pub fn ask(
        &self,
        opcode: fuse_opcode,
        nodeid: u64,
        parts: &[&[u8]],
    ) -> Result<Rc<[u8]>, Errno> {
        let asked_now = fingerprint(opcode as u32, nodeid, parts);
        let mut state = self.state.borrow_mut();
        let asker = state
            .asker
            .expect("a question to the backend outside a system call");
        let asked = state.calls.entry(asker.tid).or_default();
        if let Some(reply) = asked.replies.get(asked.used) {
            if reply.request.asked == asked_now {
                asked.used += 1;
                let (version, result) = (reply.request.version, reply.result.clone());
                state.taken = Some(version);
                return result;
            }
            let used = asked.used;
            let dropped = asked.replies.split_off(used);
            let handles = asked.handles.split_off(&used);
            state.asked_again.push(opcode as u32);
            drop(state);
            // An answer whose handle the call took gives it back as the handle goes.
            for (index, reply) in dropped.into_iter().enumerate() {
                if !handles.contains_key(&(used + index)) {
                    self.give_back(reply.request.opcode, reply.request.nodeid, &reply.result);
                }
            }
            drop(handles);
            state = self.state.borrow_mut();
        }
        let asked = state.calls.get_mut(&asker.tid).expect("asked");
        if asked.waiting.is_none() {
            state.last += 1;
            let unique = state.last;
            let request = Request {
                unique,
                opcode: opcode as u32,
                nodeid,
                asked: asked_now,
                version: state.attr_version,
            };
            state.calls.get_mut(&asker.tid).expect("asked").waiting = Some(request);
            let bytes = message(unique, opcode as u32, nodeid, &asker, parts);
            state.outbox.push(Outgoing {
                unique,
                opcode: opcode as u32,
                bytes,
                answered: true,
            });
            state.sent.insert(
                unique,
                Sent {
                    tid: Some(asker.tid),
                    opcode: opcode as u32,
                    nodeid,
                    at: 0,
                },
            );
        }
        Err(Errno::PARKED)
    }

    /// Sends a message whose answer nothing waits for.
    pub fn tell(&self, opcode: u32, nodeid: u64, parts: &[&[u8]]) {
        let mut state = self.state.borrow_mut();
        state.last += 1;
        let unique = state.last;
        let nobody = Asker {
            tid: 0,
            uid: 0,
            gid: 0,
            pid: 0,
        };
        let bytes = message(unique, opcode, nodeid, &nobody, parts);
        // FORGET and BATCH_FORGET have no answer.
        let answered = opcode != fuse_opcode::FUSE_FORGET as u32
            && opcode != fuse_opcode::FUSE_BATCH_FORGET as u32;
        state.outbox.push(Outgoing {
            unique,
            opcode,
            bytes,
            answered,
        });
        if answered {
            state.sent.insert(
                unique,
                Sent {
                    tid: None,
                    opcode,
                    nodeid,
                    at: 0,
                },
            );
        }
    }

    /// Answers a notice that asked for one (NOTIFY_REPLY, which has no answer itself).
    pub fn notify_reply(&self, unique: u64, nodeid: u64, body: &[u8]) {
        let nobody = Asker {
            tid: 0,
            uid: 0,
            gid: 0,
            pid: 0,
        };
        let opcode = fuse_opcode::FUSE_NOTIFY_REPLY as u32;
        let bytes = message(unique, opcode, nodeid, &nobody, &[body]);
        self.state.borrow_mut().outbox.push(Outgoing {
            unique,
            opcode,
            bytes,
            answered: false,
        });
    }

    /// The requests to hand to the host now, each marked sent at `now`.
    pub fn outgoing(&self, now: u64) -> Vec<Outgoing> {
        let mut state = self.state.borrow_mut();
        let out = core::mem::take(&mut state.outbox);
        for request in &out {
            if let Some(sent) = state.sent.get_mut(&request.unique) {
                sent.at = now;
            }
        }
        out
    }

    /// The backend answered request `unique`.
    pub fn answered(&self, unique: u64, reply: &[u8]) -> Answered {
        let Some((header, body)) = fuse_out_header::read_from_prefix(reply).ok() else {
            return Answered::Nobody(None);
        };
        let body = &body[..(header.len as usize).clamp(size_of::<fuse_out_header>(), reply.len())
            - size_of::<fuse_out_header>()];
        let mut state = self.state.borrow_mut();
        let Some(sent) = state.sent.remove(&unique) else {
            return Answered::Nobody(None);
        };
        let result: Result<Rc<[u8]>, Errno> = if header.error == 0 {
            Ok(Rc::from(body))
        } else {
            Err(Errno(header.error.unsigned_abs()))
        };
        if let Ok(body) = &result {
            if sent.opcode == fuse_opcode::FUSE_INIT as u32
                && let Ok((init, _)) = fuse_init_out::read_from_prefix(body)
            {
                let chunk = if init.flags & FUSE_MAX_PAGES != 0 && init.max_pages > 0 {
                    u32::from(init.max_pages) * 4096
                } else {
                    DEFAULT_CHUNK
                };
                state.settled = Some(Settled {
                    flags: init.flags,
                    max_write: init.max_write.clamp(4096, chunk.max(4096)),
                    max_read: chunk,
                });
            }
            // Each entry answered is one more lookup the backend counts.
            if sent.opcode == fuse_opcode::FUSE_READDIRPLUS as u32 {
                for (entry, _) in counted_entries(body) {
                    *state.lookups.entry(entry.nodeid).or_default() += 1;
                }
            }
            if gives_entry(sent.opcode)
                && let Ok((entry, _)) = fuse_entry_out::read_from_prefix(body)
                && entry.nodeid != 0
            {
                *state.lookups.entry(entry.nodeid).or_default() += 1;
            }
        }
        let waiter = sent.tid.filter(|tid| {
            state.calls.get(tid).is_some_and(|asked| {
                asked
                    .waiting
                    .is_some_and(|request| request.unique == unique)
            })
        });
        let Some(tid) = waiter else {
            drop(state);
            self.give_back(sent.opcode, sent.nodeid, &result);
            return Answered::Nobody(Some((sent.opcode, sent.at)));
        };
        let asked = state.calls.get_mut(&tid).expect("waiting");
        let request = asked.waiting.take().expect("waiting");
        asked.replies.push(Reply { request, result });
        Answered::Call(tid, sent.opcode, sent.at)
    }

    /// The opcodes of questions asked again since the last time, for their counter.
    pub fn asked_again(&self) -> Vec<u32> {
        core::mem::take(&mut self.state.borrow_mut().asked_again)
    }

    /// An answer no call takes gives back what it handed out: a lookup the kernel holds no node for,
    /// and an open handle.
    fn give_back(&self, opcode: u32, nodeid: u64, result: &Result<Rc<[u8]>, Errno>) {
        let Ok(body) = result else {
            return;
        };
        if opcode == fuse_opcode::FUSE_READDIRPLUS as u32 {
            for (entry, _) in counted_entries(body) {
                if !self.holds(entry.nodeid) {
                    self.forget(entry.nodeid);
                }
            }
        }
        if gives_entry(opcode)
            && let Ok((entry, _)) = fuse_entry_out::read_from_prefix(body)
            && entry.nodeid != 0
            && !self.holds(entry.nodeid)
        {
            self.forget(entry.nodeid);
        }
        let open = match opcode {
            known if known == fuse_opcode::FUSE_OPEN as u32 => Some((body.as_ref(), false)),
            known if known == fuse_opcode::FUSE_OPENDIR as u32 => Some((body.as_ref(), true)),
            known if known == fuse_opcode::FUSE_CREATE as u32 => body
                .get(size_of::<fuse_entry_out>()..)
                .map(|rest| (rest, false)),
            _ => None,
        };
        if let Some((open, directory)) = open
            && let Ok((open, _)) = fuse_open_out::read_from_prefix(open)
        {
            let nodeid = if opcode == fuse_opcode::FUSE_CREATE as u32 {
                fuse_entry_out::read_from_prefix(body).map_or(0, |(entry, _)| entry.nodeid)
            } else {
                nodeid
            };
            drop(Handle {
                client: self.me.upgrade().expect("the connection is held"),
                nodeid,
                fh: open.fh,
                flags: 0,
                open_flags: open.open_flags,
                directory,
            });
        }
    }

    /// The inode the kernel holds for a node number.
    pub fn node_of(&self, nodeid: u64) -> Option<Node> {
        self.state
            .borrow()
            .nodes
            .get(&nodeid)
            .and_then(Weak::upgrade)
    }

    fn holds(&self, nodeid: u64) -> bool {
        self.state
            .borrow()
            .nodes
            .get(&nodeid)
            .is_some_and(|node| node.strong_count() > 0)
    }

    /// The kernel holds `nodeid` no more: the backend forgets the lookups it counted.
    fn forget(&self, nodeid: u64) {
        let count = {
            let mut state = self.state.borrow_mut();
            state.nodes.remove(&nodeid);
            state.lookups.remove(&nodeid)
        };
        if let Some(count) = count {
            let forget = fuse_forget_in { nlookup: count };
            self.tell(
                fuse_opcode::FUSE_FORGET as u32,
                nodeid,
                &[forget.as_bytes()],
            );
        }
    }

    /// The inode for an entry the backend answered with, made or found again, its attributes
    /// those of the answer; a directory keeps the one it was found in.
    pub fn node(
        self: &Rc<Self>,
        entry: &fuse_entry_out,
        parent: Option<&Node>,
        name: &[u8],
    ) -> Node {
        let found = self
            .state
            .borrow()
            .nodes
            .get(&entry.nodeid)
            .and_then(Weak::upgrade);
        let node = found.unwrap_or_else(|| {
            let node = Rc::new(RefCell::new(Inode {
                fs: self.fs.clone(),
                ino: entry.attr.ino,
                mode: 0,
                uid: 0,
                gid: 0,
                nlink: 0,
                rdev: 0,
                atime: Time::default(),
                mtime: Time::default(),
                ctime: Time::default(),
                btime: Time::default(),
                place: Place {
                    parent: None,
                    name: Vec::new(),
                    mounted: None,
                    mount_root: false,
                    held_parent: None,
                },
                content: Content::Backend(Remote {
                    client: self.clone(),
                    nodeid: entry.nodeid,
                    fresh: Cell::new(0),
                    version: Cell::new(0),
                    size: Cell::new(0),
                    blocks: Cell::new(0),
                    block_size: Cell::new(0),
                    pages: None,
                    writer: Cell::new(None),
                    acls: RefCell::new([None, None]),
                }),
                flocks: Vec::new(),
                record_locks: Vec::new(),
                xattrs: BTreeMap::new(),
                fsflags: 0,
                write_hint: 0,
            }));
            self.state
                .borrow_mut()
                .nodes
                .insert(entry.nodeid, Rc::downgrade(&node));
            node
        });
        self.refresh(&node, &entry.attr);
        {
            let mut state = self.state.borrow_mut();
            if let Some(asker) = state.asker
                && let Some(asked) = state.calls.get_mut(&asker.tid)
            {
                asked.nodes.push(node.clone());
            }
        }
        if let Some(parent) = parent {
            let mut inode = node.borrow_mut();
            if !inode.place.mount_root {
                inode.place.parent = Some(Rc::downgrade(parent));
                inode.place.held_parent = Some(parent.clone());
                inode.place.name = name.to_vec();
            }
        }
        node
    }

    /// The root of the backend, numbered 1 by FUSE.
    pub fn root(self: &Rc<Self>) -> Node {
        let entry = fuse_entry_out {
            nodeid: FUSE_ROOT_ID,
            attr: fuse_attr {
                ino: FUSE_ROOT_ID,
                mode: super::S_IFDIR | 0o755,
                nlink: 2,
                ..Default::default()
            },
            ..Default::default()
        };
        self.node(&entry, None, b"")
    }

    /// Sets an inode's attributes from an answer, as fresh for this run, unless the inode changed
    /// after its request was sent (`fuse_change_attributes`), as when an earlier run of the call
    /// already took the answer.
    pub fn refresh(&self, node: &Node, attr: &fuse_attr) {
        let (run, taken) = {
            let state = self.state.borrow();
            (state.run, state.taken)
        };
        let mut inode = node.borrow_mut();
        if let Content::Backend(remote) = &inode.content
            && taken.is_some_and(|version| remote.version.get() > version)
        {
            remote.fresh.set(run);
            return;
        }
        inode.ino = attr.ino;
        inode.mode = attr.mode;
        inode.uid = attr.uid;
        inode.gid = attr.gid;
        inode.nlink = attr.nlink;
        inode.rdev = u64::from(attr.rdev);
        inode.atime = time(attr.atime, attr.atimensec);
        inode.mtime = time(attr.mtime, attr.mtimensec);
        inode.ctime = time(attr.ctime, attr.ctimensec);
        if let Content::Backend(remote) = &inode.content {
            remote.fresh.set(run);
            remote.version.set(self.next_version());
            remote.size.set(attr.size);
            remote.blocks.set(attr.blocks);
            remote.block_size.set(attr.blksize);
        }
    }

    /// A new attribute version, for a change to what an inode holds.
    fn next_version(&self) -> u64 {
        let mut state = self.state.borrow_mut();
        state.attr_version += 1;
        state.attr_version
    }

    /// Whether a listing from `offset` asks for attributes too (`fuse_use_readdirplus`): when INIT
    /// granted it, always, or with READDIRPLUS_AUTO for a listing from the start.
    pub fn uses_readdirplus(&self, offset: u64) -> bool {
        let flags = self.settled().flags;
        flags & FUSE_DO_READDIRPLUS != 0 && (flags & FUSE_READDIRPLUS_AUTO == 0 || offset == 0)
    }

    /// An entry a READDIRPLUS answer gave (`fuse_direntplus_link`): a node the kernel holds takes
    /// its attributes; one it does not is forgotten, as nothing here keeps names.
    fn link_entry(&self, entry: &fuse_entry_out, name: &[u8]) {
        if entry.nodeid == 0 || name == b"." || name == b".." {
            return;
        }
        match self.node_of(entry.nodeid) {
            Some(node) => {
                if let Content::Backend(remote) = &node.borrow().content {
                    remote.forget_acls();
                }
                self.refresh(&node, &entry.attr);
            }
            None => self.forget(entry.nodeid),
        }
    }

    /// Whether a node's attributes came from an answer this run of a call had.
    pub fn is_fresh(&self, remote: &Remote) -> bool {
        remote.fresh.get() == self.state.borrow().run
    }
}

/// Whether an answer to `opcode` is an entry the backend counts a lookup for.
fn gives_entry(opcode: u32) -> bool {
    [
        fuse_opcode::FUSE_LOOKUP,
        fuse_opcode::FUSE_MKNOD,
        fuse_opcode::FUSE_MKDIR,
        fuse_opcode::FUSE_SYMLINK,
        fuse_opcode::FUSE_LINK,
        fuse_opcode::FUSE_CREATE,
    ]
    .iter()
    .any(|known| *known as u32 == opcode)
}

fn time(seconds: u64, nanoseconds: u32) -> Time {
    Time {
        seconds: seconds as i64,
        nanoseconds,
    }
}

/// What a request asks, hashed: its opcode, node and body.
fn fingerprint(opcode: u32, nodeid: u64, parts: &[&[u8]]) -> u64 {
    use core::hash::Hasher;
    let mut hasher = siphasher::sip::SipHasher13::new();
    hasher.write_u32(opcode);
    hasher.write_u64(nodeid);
    for part in parts {
        hasher.write(part);
    }
    hasher.finish()
}

/// A request: the header, then its parts one after another.
fn message(unique: u64, opcode: u32, nodeid: u64, asker: &Asker, parts: &[&[u8]]) -> Vec<u8> {
    let length = size_of::<fuse_in_header>() + parts.iter().map(|part| part.len()).sum::<usize>();
    let header = fuse_in_header {
        len: length as u32,
        opcode,
        unique,
        nodeid,
        uid: asker.uid,
        gid: asker.gid,
        pid: asker.pid,
        total_extlen: 0,
        padding: 0,
    };
    let mut bytes = Vec::with_capacity(length);
    bytes.extend_from_slice(header.as_bytes());
    for part in parts {
        bytes.extend_from_slice(part);
    }
    bytes
}

/// The connection and node number of an inode on a backend.
pub(crate) fn remote(node: &Node) -> Option<(Rc<Client>, u64)> {
    remote_of(&node.borrow()).map(|remote| (remote.client.clone(), remote.nodeid))
}

/// The node number of an inode on a backend.
pub(crate) fn remote_number(inode: &Inode) -> u64 {
    remote_of(inode).map_or(0, |remote| remote.nodeid)
}

fn on_backend(node: &Node) -> (Rc<Client>, u64) {
    remote(node).expect("an inode on a backend")
}

/// A name as FUSE sends it, ending in a zero.
fn name_part(name: &[u8]) -> Vec<u8> {
    let mut part = Vec::with_capacity(name.len() + 1);
    part.extend_from_slice(name);
    part.push(0);
    part
}

/// The leading structure of an answer; a short answer is the backend's fault (EIO).
fn first<T: FromBytes>(body: &[u8]) -> Result<T, Errno> {
    T::read_from_prefix(body)
        .map(|(value, _)| value)
        .map_err(|_| Errno::EIO)
}

/// The inode an entry answer names, which must be of the type asked for when one is.
fn entry_node(client: &Rc<Client>, body: &[u8], parent: &Node, name: &[u8]) -> Result<Node, Errno> {
    let entry: fuse_entry_out = first(body)?;
    // A zero node is a name the backend says is not there.
    if entry.nodeid == 0 {
        return Err(Errno::ENOENT);
    }
    Ok(client.node(&entry, Some(parent), name))
}

/// `fuse_lookup`: the inode `name` names in a directory.
pub(crate) fn lookup(directory: &Node, name: &[u8]) -> Result<Node, Errno> {
    let (client, nodeid) = on_backend(directory);
    let body = client.ask(fuse_opcode::FUSE_LOOKUP, nodeid, &[&name_part(name)])?;
    let node = entry_node(&client, &body, directory, name)?;
    // `fuse_dentry_revalidate`.
    if let Some(remote) = remote_of(&node.borrow()) {
        remote.forget_acls();
    }
    Ok(node)
}

/// Asks for an inode's attributes unless this run of the call already has them
/// (`fuse_update_attributes` with no time to keep them).
pub(crate) fn fresh(node: &Node) -> Result<(), Errno> {
    let stale = match &node.borrow().content {
        Content::Backend(remote) => !remote.client.is_fresh(remote),
        _ => false,
    };
    if stale {
        getattr(node, None)?;
    }
    Ok(())
}

/// `fuse_do_getattr`: the inode's attributes, through an open handle when there is one.
pub(crate) fn getattr(node: &Node, fh: Option<u64>) -> Result<(), Errno> {
    let (client, nodeid) = on_backend(node);
    let input = fuse_getattr_in {
        getattr_flags: if fh.is_some() { FUSE_GETATTR_FH } else { 0 },
        dummy: 0,
        fh: fh.unwrap_or(0),
    };
    let body = client.ask(fuse_opcode::FUSE_GETATTR, nodeid, &[input.as_bytes()])?;
    let out: fuse_attr_out = first(&body)?;
    // `fuse_perm_getattr`, `fuse_update_get_attr`.
    if let Some(remote) = remote_of(&node.borrow()) {
        remote.forget_acls();
    }
    client.refresh(node, &out.attr);
    Ok(())
}

/// What `setattr` changes, as FUSE's `FATTR_*` bits say.
#[derive(Clone, Copy, Default)]
pub(crate) struct Change {
    pub mode: Option<u32>,
    pub uid: Option<u32>,
    pub gid: Option<u32>,
    pub size: Option<u64>,
    /// Each time: none to keep, `Some(None)` for now, or a time.
    pub atime: Option<Option<Time>>,
    pub mtime: Option<Option<Time>>,
    pub ctime: Option<Time>,
    pub fh: Option<u64>,
}

/// `fuse_do_setattr`.
pub(crate) fn setattr(node: &Node, change: Change) -> Result<(), Errno> {
    let (client, nodeid) = on_backend(node);
    let mut input = fuse_setattr_in::default();
    if let Some(mode) = change.mode {
        input.valid |= FATTR_MODE;
        input.mode = mode;
    }
    if let Some(uid) = change.uid {
        input.valid |= FATTR_UID;
        input.uid = uid;
    }
    if let Some(gid) = change.gid {
        input.valid |= FATTR_GID;
        input.gid = gid;
    }
    if let Some(size) = change.size {
        input.valid |= FATTR_SIZE;
        input.size = size;
    }
    match change.atime {
        Some(None) => input.valid |= FATTR_ATIME | FATTR_ATIME_NOW,
        Some(Some(at)) => {
            input.valid |= FATTR_ATIME;
            input.atime = at.seconds as u64;
            input.atimensec = at.nanoseconds;
        }
        None => {}
    }
    match change.mtime {
        Some(None) => input.valid |= FATTR_MTIME | FATTR_MTIME_NOW,
        Some(Some(at)) => {
            input.valid |= FATTR_MTIME;
            input.mtime = at.seconds as u64;
            input.mtimensec = at.nanoseconds;
        }
        None => {}
    }
    if let Some(at) = change.ctime {
        input.valid |= FATTR_CTIME;
        input.ctime = at.seconds as u64;
        input.ctimensec = at.nanoseconds;
    }
    if let Some(fh) = change.fh {
        input.valid |= FATTR_FH;
        input.fh = fh;
    }
    let body = client.ask(fuse_opcode::FUSE_SETATTR, nodeid, &[input.as_bytes()])?;
    let out: fuse_attr_out = first(&body)?;
    client.refresh(node, &out.attr);
    Ok(())
}

/// `fuse_readlink_page`: a symbolic link's target.
pub(crate) fn readlink(node: &Node) -> Result<Vec<u8>, Errno> {
    let (client, nodeid) = on_backend(node);
    let body = client.ask(fuse_opcode::FUSE_READLINK, nodeid, &[])?;
    Ok(body.to_vec())
}

/// The open flags FUSE sends: the access mode and status flags, without those only the kernel
/// acts on (`fuse_do_open`, `fuse_create_open`), and O_TRUNC only when the backend takes it.
fn open_flags(client: &Client, flags: u32) -> u32 {
    use crate::linux::general::{O_CREAT, O_EXCL, O_NOCTTY, O_TRUNC};
    let mut flags = flags & !(O_CREAT | O_EXCL | O_NOCTTY);
    if client.settled().flags & FUSE_ATOMIC_O_TRUNC == 0 {
        flags &= !O_TRUNC;
    }
    flags
}

/// Whether the backend truncated the file with its open (it takes FUSE_ATOMIC_O_TRUNC).
pub(crate) fn truncates_on_open(handle: &Handle) -> bool {
    handle.client.settled().flags & FUSE_ATOMIC_O_TRUNC != 0
}

/// `fuse_do_open`: opens a file or directory on the backend.
pub(crate) fn open(node: &Node, flags: u32) -> Result<Rc<Handle>, Errno> {
    let (client, nodeid) = on_backend(node);
    let directory = node.borrow().is_directory();
    let input = fuse_open_in {
        flags: open_flags(&client, flags),
        open_flags: 0,
    };
    let opcode = if directory {
        fuse_opcode::FUSE_OPENDIR
    } else {
        fuse_opcode::FUSE_OPEN
    };
    let body = client.ask(opcode, nodeid, &[input.as_bytes()])?;
    let out: fuse_open_out = first(&body)?;
    Ok(client.handle(|| Handle {
        client: client.clone(),
        nodeid,
        fh: out.fh,
        flags: input.flags,
        open_flags: out.open_flags,
        directory,
    }))
}

/// The mode a new file is asked for with, already without the umask (`mode_strip_umask`), which
/// goes along for the backend; stripped again here when the backend does not take it.
fn masked(client: &Client, mode: u32, umask: u32) -> (u32, u32) {
    if client.settled().flags & FUSE_DONT_MASK != 0 {
        (mode, umask)
    } else {
        (mode & !umask, umask)
    }
}

/// `fuse_create_open`: makes and opens a regular file.
pub(crate) fn create(
    directory: &Node,
    name: &[u8],
    mode: u32,
    umask: u32,
    flags: u32,
) -> Result<(Node, Rc<Handle>), Errno> {
    let (client, nodeid) = on_backend(directory);
    let (mode, umask) = masked(&client, mode, umask);
    let input = fuse_create_in {
        flags: open_flags(&client, flags),
        mode,
        umask,
        open_flags: 0,
    };
    let body = client.ask(
        fuse_opcode::FUSE_CREATE,
        nodeid,
        &[input.as_bytes(), &name_part(name)],
    )?;
    let out: fuse_open_out = first(body.get(size_of::<fuse_entry_out>()..).unwrap_or(&[]))?;
    let handle = client.handle(|| Handle {
        client: client.clone(),
        nodeid: first::<fuse_entry_out>(&body).map_or(0, |entry| entry.nodeid),
        fh: out.fh,
        flags: input.flags,
        open_flags: out.open_flags,
        directory: false,
    });
    let node = entry_node(&client, &body, directory, name)?;
    Ok((node, handle))
}

/// `fuse_mknod`.
pub(crate) fn mknod(
    directory: &Node,
    name: &[u8],
    mode: u32,
    rdev: u32,
    umask: u32,
) -> Result<Node, Errno> {
    let (client, nodeid) = on_backend(directory);
    let (mode, umask) = masked(&client, mode, umask);
    let input = fuse_mknod_in {
        mode,
        rdev,
        umask,
        padding: 0,
    };
    let body = client.ask(
        fuse_opcode::FUSE_MKNOD,
        nodeid,
        &[input.as_bytes(), &name_part(name)],
    )?;
    entry_node(&client, &body, directory, name)
}

/// `fuse_mkdir`.
pub(crate) fn mkdir(directory: &Node, name: &[u8], mode: u32, umask: u32) -> Result<Node, Errno> {
    let (client, nodeid) = on_backend(directory);
    let (mode, umask) = masked(&client, mode, umask);
    let input = fuse_mkdir_in { mode, umask };
    let body = client.ask(
        fuse_opcode::FUSE_MKDIR,
        nodeid,
        &[input.as_bytes(), &name_part(name)],
    )?;
    entry_node(&client, &body, directory, name)
}

/// `fuse_symlink`.
pub(crate) fn symlink(directory: &Node, name: &[u8], target: &[u8]) -> Result<Node, Errno> {
    let (client, nodeid) = on_backend(directory);
    let body = client.ask(
        fuse_opcode::FUSE_SYMLINK,
        nodeid,
        &[&name_part(name), &name_part(target)],
    )?;
    entry_node(&client, &body, directory, name)
}

/// `fuse_link`: a new name in `directory` for `node`.
pub(crate) fn link(node: &Node, directory: &Node, name: &[u8]) -> Result<Node, Errno> {
    let (client, nodeid) = on_backend(directory);
    let (_, old) = on_backend(node);
    let input = fuse_link_in { oldnodeid: old };
    let body = client.ask(
        fuse_opcode::FUSE_LINK,
        nodeid,
        &[input.as_bytes(), &name_part(name)],
    )?;
    entry_node(&client, &body, directory, name)
}

/// `fuse_unlink` and `fuse_rmdir`.
pub(crate) fn remove(directory: &Node, name: &[u8], is_directory: bool) -> Result<(), Errno> {
    let (client, nodeid) = on_backend(directory);
    let opcode = if is_directory {
        fuse_opcode::FUSE_RMDIR
    } else {
        fuse_opcode::FUSE_UNLINK
    };
    client.ask(opcode, nodeid, &[&name_part(name)])?;
    Ok(())
}

/// `fuse_rename2`: RENAME without flags, RENAME2 with them.
pub(crate) fn rename(
    from: &Node,
    old: &[u8],
    to: &Node,
    new: &[u8],
    flags: u32,
) -> Result<(), Errno> {
    let (client, nodeid) = on_backend(from);
    let (_, newdir) = on_backend(to);
    let (old, new) = (name_part(old), name_part(new));
    if flags == 0 {
        let input = fuse_rename_in { newdir };
        client.ask(
            fuse_opcode::FUSE_RENAME,
            nodeid,
            &[input.as_bytes(), &old, &new],
        )?;
    } else {
        let input = fuse_rename2_in {
            newdir,
            flags,
            padding: 0,
        };
        client.ask(
            fuse_opcode::FUSE_RENAME2,
            nodeid,
            &[input.as_bytes(), &old, &new],
        )?;
    }
    Ok(())
}

/// Reads up to `count` bytes at `offset`, in as many messages as the backend's largest takes,
/// stopping at a short answer (the end of the file).
pub(crate) fn read(handle: &Handle, offset: u64, count: u64) -> Result<Vec<u8>, Errno> {
    let chunk = u64::from(handle.client.settled().max_read);
    let mut bytes = Vec::new();
    while (bytes.len() as u64) < count {
        let size = (count - bytes.len() as u64).min(chunk) as u32;
        let input = fuse_read_in {
            fh: handle.fh,
            offset: offset + bytes.len() as u64,
            size,
            read_flags: 0,
            lock_owner: 0,
            flags: handle.flags,
            padding: 0,
        };
        let body = handle
            .client
            .ask(fuse_opcode::FUSE_READ, handle.nodeid, &[input.as_bytes()])?;
        let got = body.len().min(size as usize);
        bytes.extend_from_slice(&body[..got]);
        if got < size as usize {
            break;
        }
    }
    Ok(bytes)
}

/// Writes `bytes` at `offset`, in as many messages as the backend's largest takes; how many it took.
pub(crate) fn write(handle: &Handle, offset: u64, bytes: &[u8]) -> Result<u64, Errno> {
    let chunk = handle.client.settled().max_write as usize;
    let mut done = 0;
    while done < bytes.len() {
        let part = &bytes[done..(done + chunk).min(bytes.len())];
        let input = fuse_write_in {
            fh: handle.fh,
            offset: offset + done as u64,
            size: part.len() as u32,
            write_flags: 0,
            lock_owner: 0,
            flags: handle.flags,
            padding: 0,
        };
        let body = match handle.client.ask(
            fuse_opcode::FUSE_WRITE,
            handle.nodeid,
            &[input.as_bytes(), part],
        ) {
            Ok(body) => body,
            Err(error) if done > 0 && error != Errno::PARKED => return Ok(done as u64),
            Err(error) => return Err(error),
        };
        let out: fuse_write_out = first(&body)?;
        done += (out.size as usize).min(part.len());
        if (out.size as usize) < part.len() {
            break;
        }
    }
    Ok(done as u64)
}

/// Writes a mapped file's pages back (`fuse_writepages`), waiting for the answers or, with nothing
/// to wait, sending them on their own.
pub(crate) fn write_back(
    handle: &Handle,
    offset: u64,
    bytes: &[u8],
    wait: bool,
) -> Result<(), Errno> {
    let chunk = handle.client.settled().max_write as usize;
    for (index, part) in bytes.chunks(chunk.max(1)).enumerate() {
        let input = fuse_write_in {
            fh: handle.fh,
            offset: offset + (index * chunk) as u64,
            size: part.len() as u32,
            write_flags: FUSE_WRITE_CACHE,
            lock_owner: 0,
            flags: handle.flags,
            padding: 0,
        };
        let parts = [input.as_bytes(), part];
        if wait {
            handle
                .client
                .ask(fuse_opcode::FUSE_WRITE, handle.nodeid, &parts)?;
        } else {
            handle
                .client
                .tell(fuse_opcode::FUSE_WRITE as u32, handle.nodeid, &parts);
        }
    }
    Ok(())
}

/// `fuse_flush`: a descriptor of the file closed. Directories have no flush
/// (`fuse_dir_operations`).
pub(crate) fn flush(handle: &Handle, owner: u64) -> Result<(), Errno> {
    if handle.directory {
        return Ok(());
    }
    let input = fuse_flush_in {
        fh: handle.fh,
        unused: 0,
        padding: 0,
        lock_owner: owner,
    };
    match handle
        .client
        .ask(fuse_opcode::FUSE_FLUSH, handle.nodeid, &[input.as_bytes()])
    {
        // A backend without FLUSH has nothing to do (`fc->no_flush`).
        Ok(_) | Err(Errno::ENOSYS) => Ok(()),
        Err(error) => Err(error),
    }
}

/// `fuse_flush` for a descriptor closed by anything but `close`, whose answer nothing waits for.
pub(crate) fn flush_unwaited(handle: &Handle, owner: u64) {
    if handle.directory {
        return;
    }
    let input = fuse_flush_in {
        fh: handle.fh,
        unused: 0,
        padding: 0,
        lock_owner: owner,
    };
    handle.client.tell(
        fuse_opcode::FUSE_FLUSH as u32,
        handle.nodeid,
        &[input.as_bytes()],
    );
}

/// `fuse_fsync_common`: FSYNC, or FSYNCDIR for a directory.
pub(crate) fn fsync(handle: &Handle, data_only: bool) -> Result<(), Errno> {
    let input = fuse_fsync_in {
        fh: handle.fh,
        fsync_flags: if data_only { FUSE_FSYNC_FDATASYNC } else { 0 },
        padding: 0,
    };
    let opcode = if handle.directory {
        fuse_opcode::FUSE_FSYNCDIR
    } else {
        fuse_opcode::FUSE_FSYNC
    };
    match handle
        .client
        .ask(opcode, handle.nodeid, &[input.as_bytes()])
    {
        Ok(_) | Err(Errno::ENOSYS) => Ok(()),
        Err(error) => Err(error),
    }
}

/// `fuse_statfs`.
pub(crate) fn statfs(node: &Node) -> Result<fuse_kstatfs, Errno> {
    let (client, nodeid) = on_backend(node);
    let body = client.ask(fuse_opcode::FUSE_STATFS, nodeid, &[])?;
    let out: fuse_statfs_out = first(&body)?;
    Ok(out.st)
}

/// One directory entry READDIR gave: the node number, the position after it, its type and name.
pub(crate) struct Entry {
    pub ino: u64,
    pub next: u64,
    pub kind: u8,
    pub name: Vec<u8>,
}

/// `fuse_readdir_uncached`: the entries from position `offset` that an answer the size of the
/// caller's `count` holds, with their attributes when READDIRPLUS is used (`parse_dirplusfile`).
pub(crate) fn readdir(handle: &Handle, offset: u64, count: u64) -> Result<Vec<Entry>, Errno> {
    let client = &handle.client;
    let plus = client.uses_readdirplus(offset);
    // As much as the caller's buffer holds, from a page up to the most one message carries.
    let page = wcontainers_abi::PAGE_SIZE;
    let size = count.clamp(page, u64::from(client.settled().max_read).max(page));
    let input = fuse_read_in {
        fh: handle.fh,
        offset,
        size: size as u32,
        read_flags: 0,
        lock_owner: 0,
        flags: handle.flags,
        padding: 0,
    };
    let opcode = if plus {
        fuse_opcode::FUSE_READDIRPLUS
    } else {
        fuse_opcode::FUSE_READDIR
    };
    let body = client.ask(opcode, handle.nodeid, &[input.as_bytes()])?;
    let mut entries = Vec::new();
    let mut rest: &[u8] = &body;
    loop {
        // `parse_dirplusfile`: each entry's lookup and attributes come before its name.
        let (entry_out, after) = if plus {
            match fuse_entry_out::read_from_prefix(rest) {
                Ok((entry, after)) => (Some(entry), after),
                Err(_) => break,
            }
        } else {
            (None, rest)
        };
        let Ok((dirent, after)) = fuse_dirent::read_from_prefix(after) else {
            break;
        };
        let length = dirent.namelen as usize;
        // Each entry's name is padded to eight bytes (`FUSE_DIRENT_ALIGN`).
        let head = rest.len() - after.len();
        let record = (head + length).next_multiple_of(8);
        if record > rest.len() {
            break;
        }
        let name = &after[..length];
        if length == 0 || length > crate::linux_extra::FUSE_NAME_MAX || name.contains(&b'/') {
            return Err(Errno::EIO);
        }
        if let Some(entry_out) = &entry_out {
            client.link_entry(entry_out, name);
        }
        entries.push(Entry {
            ino: dirent.ino,
            next: dirent.off,
            kind: dirent.type_ as u8,
            name: name.to_vec(),
        });
        rest = &rest[record..];
    }
    Ok(entries)
}

/// The entries of a READDIRPLUS answer that name a node the backend counted a lookup for: all
/// but `.`, `..` and those with no node (`fuse_direntplus_link`).
fn counted_entries(body: &[u8]) -> impl Iterator<Item = (fuse_entry_out, &[u8])> {
    let mut rest = body;
    core::iter::from_fn(move || {
        loop {
            let (entry, after) = fuse_entry_out::read_from_prefix(rest).ok()?;
            let (dirent, after) = fuse_dirent::read_from_prefix(after).ok()?;
            let length = dirent.namelen as usize;
            let record = (rest.len() - after.len() + length).next_multiple_of(8);
            if record > rest.len() || length > after.len() {
                return None;
            }
            let name = &after[..length];
            rest = &rest[record..];
            if entry.nodeid != 0 && name != b"." && name != b".." {
                return Some((entry, name));
            }
        }
    })
}

/// `fuse_lseek`: the next data or hole from `offset`; a backend without LSEEK has no holes
/// (`generic_file_llseek`, after `fc->no_lseek`).
pub(crate) fn lseek(handle: &Handle, offset: i64, whence: u32) -> Result<u64, Errno> {
    use crate::linux::general::SEEK_DATA;
    let input = fuse_lseek_in {
        fh: handle.fh,
        offset: offset as u64,
        whence,
        padding: 0,
    };
    match handle
        .client
        .ask(fuse_opcode::FUSE_LSEEK, handle.nodeid, &[input.as_bytes()])
    {
        Ok(body) => Ok(first::<fuse_lseek_out>(&body)?.offset),
        Err(Errno::ENOSYS) => {
            let size = match &handle.client.node_of(handle.nodeid) {
                Some(node) => node.borrow().size(),
                None => 0,
            };
            if offset < 0 || offset as u64 >= size {
                return Err(Errno::ENXIO);
            }
            Ok(if whence == SEEK_DATA {
                offset as u64
            } else {
                size
            })
        }
        Err(error) => Err(error),
    }
}

/// `fuse_file_fallocate`'s message; a backend without FALLOCATE cannot allocate (EOPNOTSUPP).
pub(crate) fn fallocate(handle: &Handle, mode: u32, offset: u64, length: u64) -> Result<(), Errno> {
    let input = fuse_fallocate_in {
        fh: handle.fh,
        offset,
        length,
        mode,
        padding: 0,
    };
    match handle.client.ask(
        fuse_opcode::FUSE_FALLOCATE,
        handle.nodeid,
        &[input.as_bytes()],
    ) {
        Ok(_) => Ok(()),
        Err(Errno::ENOSYS) => Err(Errno::EOPNOTSUPP),
        Err(error) => Err(error),
    }
}

/// A backend without an attribute message has no attributes (`fc->no_getxattr` and the rest).
fn no_attributes(error: Errno) -> Errno {
    if error == Errno::ENOSYS {
        Errno::EOPNOTSUPP
    } else {
        error
    }
}

/// `fuse_getxattr`, and `fuse_listxattr` without a name: with `size` 0 the length the backend
/// gives, else at most `size` bytes of it (ERANGE from the backend when they do not fit).
fn ask_attributes(
    node: &Node,
    opcode: fuse_opcode,
    name: Option<&[u8]>,
    size: u32,
    largest: u32,
) -> Result<Vec<u8>, Errno> {
    let (client, nodeid) = on_backend(node);
    let input = fuse_getxattr_in { size, padding: 0 };
    let name = name.map(name_part);
    let mut parts: Vec<&[u8]> = alloc::vec![input.as_bytes()];
    parts.extend(name.as_deref());
    let body = client.ask(opcode, nodeid, &parts).map_err(no_attributes)?;
    if size == 0 {
        let out: fuse_getxattr_out = first(&body)?;
        // Only the length is asked for: the answer stands for that many bytes.
        return Ok(alloc::vec![0; out.size.min(largest) as usize]);
    }
    Ok(body[..body.len().min(size as usize)].to_vec())
}

/// `fuse_getxattr`: an attribute's value, or with `size` 0 a value of its length.
pub(crate) fn getxattr(node: &Node, name: &[u8], size: u32) -> Result<Vec<u8>, Errno> {
    use crate::linux::general::XATTR_SIZE_MAX;
    ask_attributes(
        node,
        fuse_opcode::FUSE_GETXATTR,
        Some(name),
        size,
        XATTR_SIZE_MAX,
    )
}

/// `fuse_listxattr`: the names, each ending in a zero (EIO when the backend's are not), or with
/// `size` 0 a list of their length.
pub(crate) fn listxattr(node: &Node, size: u32) -> Result<Vec<u8>, Errno> {
    use crate::linux::general::XATTR_LIST_MAX;
    let list = ask_attributes(
        node,
        fuse_opcode::FUSE_LISTXATTR,
        None,
        size,
        XATTR_LIST_MAX,
    )?;
    // `fuse_verify_xattr_list`.
    if size != 0
        && list
            .split_inclusive(|byte| *byte == 0)
            .any(|name| name.len() < 2 || name.last() != Some(&0))
    {
        return Err(Errno::EIO);
    }
    Ok(list)
}

/// `fuse_setxattr`, with the setxattr message of a backend not asked for its extended form.
pub(crate) fn setxattr(node: &Node, name: &[u8], value: &[u8], flags: u32) -> Result<(), Errno> {
    let (client, nodeid) = on_backend(node);
    let input = fuse_setxattr_in {
        size: value.len() as u32,
        flags,
        setxattr_flags: 0,
        padding: 0,
    };
    let compat = &input.as_bytes()[..FUSE_COMPAT_SETXATTR_IN_SIZE as usize];
    client
        .ask(
            fuse_opcode::FUSE_SETXATTR,
            nodeid,
            &[compat, &name_part(name), value],
        )
        .map_err(no_attributes)?;
    Ok(())
}

/// `fuse_removexattr`.
pub(crate) fn removexattr(node: &Node, name: &[u8]) -> Result<(), Errno> {
    let (client, nodeid) = on_backend(node);
    client
        .ask(fuse_opcode::FUSE_REMOVEXATTR, nodeid, &[&name_part(name)])
        .map_err(no_attributes)?;
    Ok(())
}

impl crate::Kernel {
    /// Hands the backend the messages made since the last time, counting each by its opcode.
    pub(crate) fn send_to_backend(&mut self, cx: &mut crate::Context<'_>) {
        let Some(client) = self.files.backend.clone() else {
            return;
        };
        for opcode in client.asked_again() {
            self.counters.count(
                cx.host,
                crate::reason::BACKEND_ASKED_AGAIN,
                u64::from(opcode),
            );
        }
        let now = cx.monotonic();
        for request in client.outgoing(now) {
            // A message with no answer is counted when it goes; the rest when answered, with
            // the time the answer took.
            if !request.answered {
                self.counters
                    .measure(crate::reason::BACKEND_MESSAGE, u64::from(request.opcode), 0);
            }
            let id = wcontainers_abi::BackendRequestId(request.unique);
            if cx.host.backend_send(id, &request.bytes).is_err() {
                self.counters.count(
                    cx.host,
                    crate::reason::BACKEND_UNREACHABLE,
                    u64::from(request.opcode),
                );
                // As from a connection that is gone: the call that waits gets ENOTCONN.
                let header = fuse_out_header {
                    len: size_of::<fuse_out_header>() as u32,
                    error: -(crate::linux::errno::ENOTCONN as i32),
                    unique: request.unique,
                };
                self.backend_answered(cx, request.unique, header.as_bytes());
            }
        }
    }

    /// The backend answered: the call waiting for it runs again.
    pub(crate) fn backend_answered(
        &mut self,
        cx: &mut crate::Context<'_>,
        unique: u64,
        reply: &[u8],
    ) {
        let Some(client) = self.files.backend.clone() else {
            self.counters
                .count(cx.host, crate::reason::BACKEND_UNEXPECTED, unique);
            return;
        };
        let now = self.entry_now;
        match client.answered(unique, reply) {
            Answered::Call(tid, opcode, at) => {
                self.counters.measure(
                    crate::reason::BACKEND_MESSAGE,
                    u64::from(opcode),
                    now.saturating_sub(at),
                );
                if self.threads.get(&tid).is_some_and(|thread| {
                    thread.call.wait == Some(crate::process::Channel::Backend)
                }) {
                    self.wake_thread(tid);
                }
            }
            Answered::Nobody(Some((opcode, at))) => {
                self.counters.measure(
                    crate::reason::BACKEND_MESSAGE,
                    u64::from(opcode),
                    now.saturating_sub(at),
                );
            }
            Answered::Nobody(None) => {
                self.counters
                    .count(cx.host, crate::reason::BACKEND_UNEXPECTED, unique);
            }
        }
    }

    /// A notice the backend sent unasked (`fuse_notify`). Nothing is cached, so most change only
    /// what the kernel holds of an inode, or the pages of a mapped file.
    pub(crate) fn backend_notice(&mut self, cx: &mut crate::Context<'_>, notice: &[u8]) {
        let Some(client) = self.files.backend.clone() else {
            return;
        };
        let Ok((header, body)) = fuse_out_header::read_from_prefix(notice) else {
            self.counters
                .count(cx.host, crate::reason::BACKEND_UNEXPECTED, 0);
            return;
        };
        let code = header.error;
        // The notice's own size, as `fuse_notify` reads it.
        let size = (header.len as usize)
            .saturating_sub(size_of::<fuse_out_header>())
            .min(body.len());
        let body = &body[..size];
        // Each notice's body must have its exact size (`fuse_notify_*`), else EINVAL.
        fn exact<T: FromBytes>(body: &[u8]) -> Result<T, Errno> {
            if body.len() != size_of::<T>() {
                return Err(Errno::EINVAL);
            }
            first(body)
        }
        // A notice about a name: its header, then the name and a zero byte.
        fn named<T: FromBytes>(
            body: &[u8],
            namelen: impl Fn(&T) -> u32,
        ) -> Result<(T, &[u8]), Errno> {
            let (out, rest) = T::read_from_prefix(body).map_err(|_| Errno::EINVAL)?;
            let length = namelen(&out) as usize;
            if length > crate::linux_extra::FUSE_NAME_MAX {
                return Err(Errno::ENAMETOOLONG);
            }
            if rest.len() != length + 1 {
                return Err(Errno::EINVAL);
            }
            Ok((out, &rest[..length]))
        }
        let handled: Option<Result<(), Errno>> = match code {
            // A poll's readiness changed: every wait on readiness looks again.
            code if code == fuse_notify_code::FUSE_NOTIFY_POLL as i32 => {
                Some(exact::<fuse_notify_poll_wakeup_out>(body).map(|_| {
                    self.wake(crate::process::Channel::Readiness);
                }))
            }
            // An inode's attributes or bytes changed: what this run of a call had of its
            // attributes is stale. A mapped file's pages keep their bytes (section 15 of the plan),
            // where Linux would read them again.
            code if code == fuse_notify_code::FUSE_NOTIFY_INVAL_INODE as i32 => {
                Some(exact::<fuse_notify_inval_inode_out>(body).map(|out| {
                    if let Some(node) = client.node_of(out.ino)
                        && let Content::Backend(remote) = &node.borrow().content
                    {
                        remote.fresh.set(0);
                        remote.changed();
                        remote.forget_acls();
                        if out.off >= 0 && remote.pages.is_some() {
                            self.counters.count(
                                cx.host,
                                crate::reason::BACKEND_CHANGE_NOT_MAPPED,
                                out.ino,
                            );
                        }
                    }
                }))
            }
            // A name changed: no names are kept.
            code if code == fuse_notify_code::FUSE_NOTIFY_INVAL_ENTRY as i32 => {
                Some(named::<fuse_notify_inval_entry_out>(body, |out| out.namelen).map(|_| ()))
            }
            // A name is gone: the inode it named, if held, has no links left
            // (`fuse_reverse_inval_entry`'s `clear_nlink`), once the parent is a directory and
            // the name the child's, where the kernel knows them.
            code if code == fuse_notify_code::FUSE_NOTIFY_DELETE as i32 => Some(
                named::<fuse_notify_delete_out>(body, |out| out.namelen).and_then(|(out, name)| {
                    let parent = client.node_of(out.parent);
                    if parent
                        .as_ref()
                        .is_some_and(|parent| !parent.borrow().is_directory())
                    {
                        return Err(Errno::ENOTDIR);
                    }
                    let Some(child) = client.node_of(out.child) else {
                        return Ok(());
                    };
                    let mut inode = child.borrow_mut();
                    if let Some(above) = inode.place.parent.as_ref().and_then(Weak::upgrade) {
                        let same_parent = parent
                            .as_ref()
                            .is_some_and(|parent| Rc::ptr_eq(parent, &above));
                        if !same_parent || inode.place.name != name {
                            return Err(Errno::ENOENT);
                        }
                    }
                    inode.nlink = 0;
                    Ok(())
                }),
            ),
            // Bytes to keep (`fuse_notify_store`): a mapped file's pages take them, and the file
            // ends at least where they do; otherwise nothing is kept and the backend has them.
            code if code == fuse_notify_code::FUSE_NOTIFY_STORE as i32 => {
                Some(match fuse_notify_store_out::read_from_prefix(body) {
                    Ok((out, data)) if data.len() == out.size as usize => {
                        if let Some(node) = client.node_of(out.nodeid)
                            && node.borrow().content.block().is_some()
                        {
                            crate::shared::backend_written(cx, &node, out.offset, data);
                            if let Content::Backend(remote) = &node.borrow().content {
                                let end = out.offset + data.len() as u64;
                                remote.set_size(remote.size.get().max(end));
                            }
                            let _ = crate::shared::changed(self, cx, &node);
                        }
                        Ok(())
                    }
                    _ => Err(Errno::EINVAL),
                })
            }
            // The backend asks for the bytes kept (`fuse_retrieve`): a mapped file's, up to its
            // end; none of any other file.
            code if code == fuse_notify_code::FUSE_NOTIFY_RETRIEVE as i32 => {
                Some(exact::<fuse_notify_retrieve_out>(body).map(|out| {
                    let bytes = client
                        .node_of(out.nodeid)
                        .and_then(|node| {
                            let size = node.borrow().size();
                            crate::shared::backend_bytes(
                                cx,
                                &node,
                                out.offset,
                                u64::from(out.size),
                                size,
                            )
                        })
                        .unwrap_or_default();
                    let reply = fuse_notify_retrieve_in {
                        dummy1: 0,
                        offset: out.offset,
                        size: bytes.len() as u32,
                        dummy2: 0,
                        dummy3: 0,
                        dummy4: 0,
                    };
                    let mut body = reply.as_bytes().to_vec();
                    body.extend_from_slice(&bytes);
                    client.notify_reply(out.notify_unique, out.nodeid, &body);
                }))
            }
            _ => None,
        };
        match handled {
            Some(Ok(())) => self
                .counters
                .measure(crate::reason::BACKEND_NOTICE, code as u64, 0),
            Some(Err(errno)) => self.counters.count(
                cx.host,
                crate::reason::BACKEND_NOTICE_REFUSED,
                ((code as u64) << 32) | u64::from(errno.0),
            ),
            None => self
                .counters
                .count(cx.host, crate::reason::BACKEND_UNEXPECTED, code as u64),
        }
    }
}
