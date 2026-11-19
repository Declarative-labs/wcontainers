//! The passthrough file system announcing writes of many pages (BIG_WRITES and MAX_PAGES), which
//! it serves but does not offer itself, and that a file's cached pages are dropped when its
//! modification time changes (AUTO_INVAL_DATA, which libfuse's servers ask for by default); every
//! other operation is passed through unchanged.

use std::ffi::CStr;
use std::io;
use std::time::Duration;

use fuse_backend_rs::abi::fuse_abi::{
    CreateIn, FsOptions, OpenOptions, SetattrValid, stat64, statvfs64,
};
use fuse_backend_rs::api::filesystem::{
    Context, DirEntry, Entry, FileLock, FileSystem, GetxattrReply, IoctlData, ListxattrReply,
    ZeroCopyReader, ZeroCopyWriter,
};
use fuse_backend_rs::passthrough::PassthroughFs;

pub struct Announcing(pub PassthroughFs);

impl FileSystem for Announcing {
    type Inode = <PassthroughFs as FileSystem>::Inode;
    type Handle = <PassthroughFs as FileSystem>::Handle;

    fn init(&self, capable: FsOptions) -> io::Result<FsOptions> {
        let wanted = self.0.init(capable)?;
        Ok(wanted
            | (capable
                & (FsOptions::BIG_WRITES | FsOptions::MAX_PAGES | FsOptions::AUTO_INVAL_DATA)))
    }

    fn destroy(&self) {
        self.0.destroy()
    }

    fn lookup(&self, ctx: &Context, parent: Self::Inode, name: &CStr) -> io::Result<Entry> {
        self.0.lookup(ctx, parent, name)
    }

    fn forget(&self, ctx: &Context, inode: Self::Inode, count: u64) {
        self.0.forget(ctx, inode, count)
    }

    fn batch_forget(&self, ctx: &Context, requests: Vec<(Self::Inode, u64)>) {
        self.0.batch_forget(ctx, requests)
    }

    fn getattr(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        handle: Option<Self::Handle>,
    ) -> io::Result<(stat64, Duration)> {
        self.0.getattr(ctx, inode, handle)
    }

    fn setattr(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        attr: stat64,
        handle: Option<Self::Handle>,
        valid: SetattrValid,
    ) -> io::Result<(stat64, Duration)> {
        self.0.setattr(ctx, inode, attr, handle, valid)
    }

    fn readlink(&self, ctx: &Context, inode: Self::Inode) -> io::Result<Vec<u8>> {
        self.0.readlink(ctx, inode)
    }

    fn symlink(
        &self,
        ctx: &Context,
        linkname: &CStr,
        parent: Self::Inode,
        name: &CStr,
    ) -> io::Result<Entry> {
        self.0.symlink(ctx, linkname, parent, name)
    }

    fn mknod(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        name: &CStr,
        mode: u32,
        rdev: u32,
        umask: u32,
    ) -> io::Result<Entry> {
        self.0.mknod(ctx, inode, name, mode, rdev, umask)
    }

    fn mkdir(
        &self,
        ctx: &Context,
        parent: Self::Inode,
        name: &CStr,
        mode: u32,
        umask: u32,
    ) -> io::Result<Entry> {
        self.0.mkdir(ctx, parent, name, mode, umask)
    }

    fn unlink(&self, ctx: &Context, parent: Self::Inode, name: &CStr) -> io::Result<()> {
        self.0.unlink(ctx, parent, name)
    }

    fn rmdir(&self, ctx: &Context, parent: Self::Inode, name: &CStr) -> io::Result<()> {
        self.0.rmdir(ctx, parent, name)
    }

    fn rename(
        &self,
        ctx: &Context,
        olddir: Self::Inode,
        oldname: &CStr,
        newdir: Self::Inode,
        newname: &CStr,
        flags: u32,
    ) -> io::Result<()> {
        self.0.rename(ctx, olddir, oldname, newdir, newname, flags)
    }

    fn link(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        newparent: Self::Inode,
        newname: &CStr,
    ) -> io::Result<Entry> {
        self.0.link(ctx, inode, newparent, newname)
    }

    fn open(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        flags: u32,
        fuse_flags: u32,
    ) -> io::Result<(Option<Self::Handle>, OpenOptions, Option<u32>)> {
        self.0.open(ctx, inode, flags, fuse_flags)
    }

    fn create(
        &self,
        ctx: &Context,
        parent: Self::Inode,
        name: &CStr,
        args: CreateIn,
    ) -> io::Result<(Entry, Option<Self::Handle>, OpenOptions, Option<u32>)> {
        self.0.create(ctx, parent, name, args)
    }

    fn read(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        handle: Self::Handle,
        w: &mut dyn ZeroCopyWriter,
        size: u32,
        offset: u64,
        lock_owner: Option<u64>,
        flags: u32,
    ) -> io::Result<usize> {
        self.0
            .read(ctx, inode, handle, w, size, offset, lock_owner, flags)
    }

    #[allow(clippy::too_many_arguments)]
    fn write(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        handle: Self::Handle,
        r: &mut dyn ZeroCopyReader,
        size: u32,
        offset: u64,
        lock_owner: Option<u64>,
        delayed_write: bool,
        flags: u32,
        fuse_flags: u32,
    ) -> io::Result<usize> {
        self.0.write(
            ctx,
            inode,
            handle,
            r,
            size,
            offset,
            lock_owner,
            delayed_write,
            flags,
            fuse_flags,
        )
    }

    fn flush(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        handle: Self::Handle,
        lock_owner: u64,
    ) -> io::Result<()> {
        self.0.flush(ctx, inode, handle, lock_owner)
    }

    fn fsync(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        datasync: bool,
        handle: Self::Handle,
    ) -> io::Result<()> {
        self.0.fsync(ctx, inode, datasync, handle)
    }

    fn fallocate(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        handle: Self::Handle,
        mode: u32,
        offset: u64,
        length: u64,
    ) -> io::Result<()> {
        self.0.fallocate(ctx, inode, handle, mode, offset, length)
    }

    #[allow(clippy::too_many_arguments)]
    fn release(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        flags: u32,
        handle: Self::Handle,
        flush: bool,
        flock_release: bool,
        lock_owner: Option<u64>,
    ) -> io::Result<()> {
        self.0
            .release(ctx, inode, flags, handle, flush, flock_release, lock_owner)
    }

    fn statfs(&self, ctx: &Context, inode: Self::Inode) -> io::Result<statvfs64> {
        self.0.statfs(ctx, inode)
    }

    fn setxattr(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        name: &CStr,
        value: &[u8],
        flags: u32,
    ) -> io::Result<()> {
        self.0.setxattr(ctx, inode, name, value, flags)
    }

    fn getxattr(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        name: &CStr,
        size: u32,
    ) -> io::Result<GetxattrReply> {
        self.0.getxattr(ctx, inode, name, size)
    }

    fn listxattr(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        size: u32,
    ) -> io::Result<ListxattrReply> {
        self.0.listxattr(ctx, inode, size)
    }

    fn removexattr(&self, ctx: &Context, inode: Self::Inode, name: &CStr) -> io::Result<()> {
        self.0.removexattr(ctx, inode, name)
    }

    fn opendir(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        flags: u32,
    ) -> io::Result<(Option<Self::Handle>, OpenOptions)> {
        self.0.opendir(ctx, inode, flags)
    }

    fn readdir(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        handle: Self::Handle,
        size: u32,
        offset: u64,
        add_entry: &mut dyn FnMut(DirEntry) -> io::Result<usize>,
    ) -> io::Result<()> {
        self.0.readdir(ctx, inode, handle, size, offset, add_entry)
    }

    fn readdirplus(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        handle: Self::Handle,
        size: u32,
        offset: u64,
        add_entry: &mut dyn FnMut(DirEntry, Entry) -> io::Result<usize>,
    ) -> io::Result<()> {
        self.0
            .readdirplus(ctx, inode, handle, size, offset, add_entry)
    }

    fn fsyncdir(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        datasync: bool,
        handle: Self::Handle,
    ) -> io::Result<()> {
        self.0.fsyncdir(ctx, inode, datasync, handle)
    }

    fn releasedir(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        flags: u32,
        handle: Self::Handle,
    ) -> io::Result<()> {
        self.0.releasedir(ctx, inode, flags, handle)
    }

    fn access(&self, ctx: &Context, inode: Self::Inode, mask: u32) -> io::Result<()> {
        self.0.access(ctx, inode, mask)
    }

    fn lseek(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        handle: Self::Handle,
        offset: u64,
        whence: u32,
    ) -> io::Result<u64> {
        self.0.lseek(ctx, inode, handle, offset, whence)
    }

    /// Query file lock status
    fn getlk(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        handle: Self::Handle,
        owner: u64,
        lock: FileLock,
        flags: u32,
    ) -> io::Result<FileLock> {
        self.0.getlk(ctx, inode, handle, owner, lock, flags)
    }

    /// Grab a file read lock
    fn setlk(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        handle: Self::Handle,
        owner: u64,
        lock: FileLock,
        flags: u32,
    ) -> io::Result<()> {
        self.0.setlk(ctx, inode, handle, owner, lock, flags)
    }

    /// Grab a file write lock
    fn setlkw(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        handle: Self::Handle,
        owner: u64,
        lock: FileLock,
        flags: u32,
    ) -> io::Result<()> {
        self.0.setlkw(ctx, inode, handle, owner, lock, flags)
    }

    /// send ioctl to the file
    #[allow(clippy::too_many_arguments)]
    fn ioctl(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        handle: Self::Handle,
        flags: u32,
        cmd: u32,
        data: IoctlData,
        out_size: u32,
    ) -> io::Result<IoctlData<'_>> {
        self.0.ioctl(ctx, inode, handle, flags, cmd, data, out_size)
    }

    /// Query a file's block mapping info
    fn bmap(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        block: u64,
        blocksize: u32,
    ) -> io::Result<u64> {
        self.0.bmap(ctx, inode, block, blocksize)
    }

    /// Poll a file's events
    fn poll(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        handle: Self::Handle,
        khandle: Self::Handle,
        flags: u32,
        events: u32,
    ) -> io::Result<u32> {
        self.0.poll(ctx, inode, handle, khandle, flags, events)
    }

    /// Send notify reply.
    fn notify_reply(&self) -> io::Result<()> {
        self.0.notify_reply()
    }

    #[inline]
    fn id_remap(&self, ctx: &mut Context) -> io::Result<()> {
        self.0.id_remap(ctx)
    }
}
