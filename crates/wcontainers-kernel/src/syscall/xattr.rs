//! Extended attributes, as Linux's xattr layer, its capability checks and tmpfs's handlers (`security.`,
//! `trusted.` and `user.` names) give them. tmpfs here has no POSIX ACLs, as in the reference.

use alloc::vec::Vec;

use wcontainers_abi::PAGE_SIZE;

use super::{Args, guest};
use crate::credentials::{CAP_SETFCAP, CAP_SYS_ADMIN, Credentials};
use crate::fs::{Content, Inode, Kind, Node};
use crate::linux::general::*;
use crate::linux_extra::{
    ACL_GROUP, ACL_GROUP_OBJ, ACL_MASK, ACL_OTHER, ACL_PERMISSIONS, ACL_UNDEFINED_ID, ACL_USER,
    ACL_USER_OBJ, POSIX_ACL_XATTR_VERSION,
};
use crate::process::Tid;
use crate::{Context, Errno, Kernel};

/// The name file capabilities are kept under.
pub(crate) const CAPABILITY: &[u8] = b"security.capability";
const ACL_ACCESS: &[u8] = b"system.posix_acl_access";
const ACL_DEFAULT: &[u8] = b"system.posix_acl_default";
/// The sizes of the two forms of file capabilities (`XATTR_CAPS_SZ_2`, `XATTR_CAPS_SZ_3`).
const CAPS_V2: usize = size_of::<vfs_cap_data>();
const CAPS_V3: usize = size_of::<vfs_ns_cap_data>();

fn prefix(constant: &'static [u8]) -> &'static [u8] {
    &constant[..constant.len() - 1]
}

/// What an attribute call names: a path, followed or not, or an open file.
enum Target {
    Path { path: u64, follow: bool },
    File(u64),
}

/// The inode a call names.
fn target(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    target: Target,
) -> Result<Node, Errno> {
    let node = match target {
        Target::Path { path, follow } => {
            super::files::lookup(kernel, cx, tid, AT_FDCWD as u64, path, 0, follow)
        }
        Target::File(fd) => {
            let file = kernel.files(tid).file(fd as i32)?;
            if file.borrow().is_path() {
                return Err(Errno::EBADF);
            }
            let node = file.borrow().node.clone();
            Ok(node)
        }
    }?;
    Ok(node)
}

/// Whether an inode's attributes are the backend's.
fn on_backend(node: &Node) -> bool {
    crate::fs::backend::remote(node).is_some()
}

/// `do_get_acl` on the backend: `__fuse_get_acl` asks for the list into a page; none or ENODATA
/// is no list, ERANGE is E2BIG, and a list comes back as `posix_acl_to_xattr` writes it.
fn backend_acl(node: &Node, name: &[u8]) -> Result<Vec<u8>, Errno> {
    let value = match crate::fs::backend::getxattr(node, name, PAGE_SIZE as u32) {
        Ok(value) if !value.is_empty() => value,
        Ok(_) | Err(Errno::ENODATA) => return Err(Errno::ENODATA),
        Err(Errno::ERANGE) => return Err(Errno::E2BIG),
        Err(error) => return Err(error),
    };
    check_acl(&value)?;
    if value.len() <= 4 {
        return Err(Errno::ENODATA);
    }
    Ok(acl_normalized(&value))
}

/// Linux's `posix_acl_valid`: the entries in order, the owner's, users', the group's, groups',
/// the mask (needed after any user or group) and the others', with known permission bits.
fn acl_valid(value: &[u8]) -> Result<(), Errno> {
    let mut state = ACL_USER_OBJ;
    let mut needs_mask = false;
    for entry in value[4..].chunks(8) {
        let tag = u16::from_le_bytes([entry[0], entry[1]]);
        let permissions = u16::from_le_bytes([entry[2], entry[3]]);
        if permissions & !ACL_PERMISSIONS != 0 {
            return Err(Errno::EINVAL);
        }
        state = match (tag, state) {
            (ACL_USER_OBJ, ACL_USER_OBJ) => ACL_USER,
            (ACL_USER, ACL_USER) | (ACL_GROUP, ACL_GROUP) => {
                needs_mask = true;
                state
            }
            (ACL_GROUP_OBJ, ACL_USER) => ACL_GROUP,
            (ACL_MASK, ACL_GROUP) => ACL_OTHER,
            (ACL_OTHER, ACL_OTHER) => 0,
            (ACL_OTHER, ACL_GROUP) if !needs_mask => 0,
            _ => return Err(Errno::EINVAL),
        };
    }
    if state == 0 {
        Ok(())
    } else {
        Err(Errno::EINVAL)
    }
}

/// An access control list as `posix_acl_to_xattr` writes it back: entries other than a user's or
/// group's name no id.
fn acl_normalized(value: &[u8]) -> Vec<u8> {
    let mut out = value.to_vec();
    for entry in out[4..].chunks_mut(8) {
        let tag = u16::from_le_bytes([entry[0], entry[1]]);
        if tag != ACL_USER && tag != ACL_GROUP {
            entry[4..8].copy_from_slice(&ACL_UNDEFINED_ID.to_le_bytes());
        }
    }
    out
}

/// `vfs_set_acl` and `vfs_remove_acl` on the backend, past the checks of the value's layout:
/// `set_posix_acl` (a FUSE mount takes lists, SB_POSIXACL) then `fuse_set_acl`, which hands the
/// list to the backend as an attribute, or removes it, since INIT does not ask for FUSE_POSIX_ACL
/// and the mount is the host's (`fuse_no_acl`). An empty value or a bare header removes it.
fn set_backend_acl(
    kernel: &Kernel,
    tid: Tid,
    node: &Node,
    name: &[u8],
    value: &[u8],
) -> Result<u64, Errno> {
    let list = value.len() > 4;
    if list {
        acl_valid(value)?;
    }
    {
        let inode = node.borrow();
        if name == ACL_DEFAULT && !inode.is_directory() {
            return if list { Err(Errno::EACCES) } else { Ok(0) };
        }
        if !kernel.credentials(tid).owns_or_capable(inode.uid) {
            return Err(Errno::EPERM);
        }
    }
    if list {
        crate::fs::backend::setxattr(node, name, &acl_normalized(value), 0)?;
    } else {
        crate::fs::backend::removexattr(node, name)?;
    }
    Ok(0)
}

/// An attribute name, as Linux's `import_xattr_name` copies it.
fn import_name(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    address: u64,
) -> Result<Vec<u8>, Errno> {
    match guest(kernel, cx, tid).read_string(address, u64::from(XATTR_NAME_MAX) + 1) {
        Ok(name) if name.is_empty() => Err(Errno::ERANGE),
        Ok(name) => Ok(name),
        Err(Errno::ENAMETOOLONG) => Err(Errno::ERANGE),
        Err(error) => Err(error),
    }
}

/// Whether an inode keeps attributes at all (Linux's `IOP_XATTR`): only tmpfs's and the
/// backend's do.
fn keeps_attributes(inode: &Inode) -> bool {
    matches!(inode.fs.kind, Kind::Tmpfs | Kind::Fuse)
}

/// Linux's `xattr_resolve_name` over tmpfs's handlers: a name must have a known prefix and something
/// after it. The backend's one handler takes every name (`fuse_xattr_handler`).
fn resolve_name(inode: &Inode, name: &[u8]) -> Result<(), Errno> {
    if !keeps_attributes(inode) {
        return Err(Errno::EOPNOTSUPP);
    }
    if inode.fs.kind == Kind::Fuse {
        return Ok(());
    }
    for known in [
        XATTR_SECURITY_PREFIX.as_slice(),
        XATTR_TRUSTED_PREFIX,
        XATTR_USER_PREFIX,
    ] {
        if let Some(rest) = name.strip_prefix(prefix(known)) {
            return if rest.is_empty() {
                Err(Errno::EINVAL)
            } else {
                Ok(())
            };
        }
    }
    Err(Errno::EOPNOTSUPP)
}

/// Linux's `may_write_xattr`: a process's directory in `/proc` is immutable.
fn may_write(inode: &Inode) -> Result<(), Errno> {
    if matches!(
        inode.content,
        Content::Proc(crate::fs::proc::Entry::Process(_))
    ) {
        return Err(Errno::EPERM);
    }
    Ok(())
}

/// Linux's `xattr_permission`.
fn permission(
    inode: &Inode,
    credentials: &Credentials,
    caller: crate::process::Pid,
    name: &[u8],
    write: bool,
) -> Result<(), Errno> {
    if write {
        may_write(inode)?;
    }
    if name.starts_with(prefix(XATTR_SECURITY_PREFIX))
        || name.starts_with(prefix(XATTR_SYSTEM_PREFIX))
    {
        return Ok(());
    }
    let refused = if write { Errno::EPERM } else { Errno::ENODATA };
    if name.starts_with(prefix(XATTR_TRUSTED_PREFIX)) {
        return if credentials.has(CAP_SYS_ADMIN) {
            Ok(())
        } else {
            Err(refused)
        };
    }
    if name.starts_with(prefix(XATTR_USER_PREFIX)) {
        let file_type = inode.file_type();
        if file_type != S_IFREG && file_type != S_IFDIR {
            return Err(refused);
        }
        if file_type == S_IFDIR
            && inode.mode & S_ISVTX != 0
            && write
            && !credentials.owns_or_capable(inode.uid)
        {
            return Err(Errno::EPERM);
        }
    }
    crate::fs::permission(inode, credentials, if write { W_OK } else { R_OK }, caller)
}

/// The capability layer's check on changing or removing a `security.` name (`cap_inode_setxattr`,
/// `cap_inode_removexattr`).
fn security_check(credentials: &Credentials, name: &[u8], removing: bool) -> Result<(), Errno> {
    if !name.starts_with(prefix(XATTR_SECURITY_PREFIX)) {
        return Ok(());
    }
    let needed = if name == CAPABILITY {
        if !removing {
            return Ok(());
        }
        CAP_SETFCAP
    } else {
        CAP_SYS_ADMIN
    };
    if credentials.has(needed) {
        Ok(())
    } else {
        Err(Errno::EPERM)
    }
}

/// File capabilities as they are stored, from what a program gives (Linux's `cap_convert_nscap`): the
/// short form is kept, the long form is kept with its owner.
fn convert_capabilities(credentials: &Credentials, value: &[u8]) -> Result<Vec<u8>, Errno> {
    let revision = capability_revision(value);
    let valid = (value.len() == CAPS_V2 && revision == VFS_CAP_REVISION_2)
        || (value.len() == CAPS_V3 && revision == VFS_CAP_REVISION_3);
    if !valid {
        return Err(Errno::EINVAL);
    }
    if !credentials.has(CAP_SETFCAP) {
        return Err(Errno::EPERM);
    }
    if value.len() == CAPS_V2 {
        return Ok(value.to_vec());
    }
    let root = u32::from_le_bytes(value[CAPS_V2..].try_into().expect("four bytes"));
    if root == u32::MAX {
        return Err(Errno::EINVAL);
    }
    let magic = VFS_CAP_REVISION_3 | (magic(value) & VFS_CAP_FLAGS_EFFECTIVE);
    let mut stored = magic.to_le_bytes().to_vec();
    stored.extend_from_slice(&value[4..CAPS_V2]);
    stored.extend_from_slice(&root.to_le_bytes());
    Ok(stored)
}

fn magic(value: &[u8]) -> u32 {
    u32::from_le_bytes(value[..4].try_into().expect("four bytes"))
}

fn capability_revision(value: &[u8]) -> u32 {
    if value.len() < 4 {
        return 0;
    }
    magic(value) & VFS_CAP_REVISION_MASK
}

/// Stored file capabilities as a reader sees them (Linux's `cap_inode_getsecurity`): owned by root,
/// the short form; owned by another user, the long form naming that user.
fn read_capabilities(stored: &[u8]) -> Result<Vec<u8>, Errno> {
    let revision = capability_revision(stored);
    let root = if stored.len() == CAPS_V2 && revision == VFS_CAP_REVISION_2 {
        0
    } else if stored.len() == CAPS_V3 && revision == VFS_CAP_REVISION_3 {
        u32::from_le_bytes(stored[CAPS_V2..].try_into().expect("four bytes"))
    } else {
        return Err(Errno::EINVAL);
    };
    if root == u32::MAX {
        return Err(Errno::EOVERFLOW);
    }
    if root != 0 {
        let mut value = stored.to_vec();
        if value.len() == CAPS_V2 {
            let magic = VFS_CAP_REVISION_3 | (magic(stored) & VFS_CAP_FLAGS_EFFECTIVE);
            value[..4].copy_from_slice(&magic.to_le_bytes());
            value.extend_from_slice(&root.to_le_bytes());
        }
        return Ok(value);
    }
    let magic = VFS_CAP_REVISION_2 | (magic(stored) & VFS_CAP_FLAGS_EFFECTIVE);
    let mut value = magic.to_le_bytes().to_vec();
    value.extend_from_slice(&stored[4..CAPS_V2]);
    Ok(value)
}

/// Checks an access control list's layout as Linux's `posix_acl_from_xattr` does. tmpfs here keeps no
/// lists, so even a valid one is then refused.
fn check_acl(value: &[u8]) -> Result<(), Errno> {
    const HEADER: usize = 4;
    const ENTRY: usize = 8;
    if value.is_empty() {
        return Ok(());
    }
    if value.len() < HEADER {
        return Err(Errno::EINVAL);
    }
    if magic(value) != POSIX_ACL_XATTR_VERSION {
        return Err(Errno::EOPNOTSUPP);
    }
    if !(value.len() - HEADER).is_multiple_of(ENTRY) {
        return Err(Errno::EINVAL);
    }
    for entry in value[HEADER..].chunks(ENTRY) {
        let tag = u16::from_le_bytes([entry[0], entry[1]]);
        let id = u32::from_le_bytes(entry[4..8].try_into().expect("four bytes"));
        match tag {
            ACL_USER_OBJ | ACL_GROUP_OBJ | ACL_MASK | ACL_OTHER => {}
            ACL_USER | ACL_GROUP if id != u32::MAX => {}
            _ => return Err(Errno::EINVAL),
        }
    }
    Ok(())
}

fn set(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    at: Target,
    [_, name, value, size, flags, _]: Args,
) -> Result<u64, Errno> {
    // In the order of Linux's `setxattr_copy`, the lookup, `mnt_want_write` and `vfs_setxattr`.
    let flags = flags as u32;
    if flags & !(XATTR_CREATE | XATTR_REPLACE) != 0 {
        return Err(Errno::EINVAL);
    }
    let name = import_name(kernel, cx, tid, name)?;
    if size > u64::from(XATTR_SIZE_MAX) {
        return Err(Errno::E2BIG);
    }
    let value = guest(kernel, cx, tid).read(value, size)?;
    let node = target(kernel, cx, tid, at)?;
    if node.borrow().fs.read_only() {
        return Err(Errno::EROFS);
    }
    if name == ACL_ACCESS || name == ACL_DEFAULT {
        check_acl(&value)?;
        may_write(&node.borrow())?;
        if on_backend(&node) {
            return set_backend_acl(kernel, tid, &node, &name, &value);
        }
        return Err(Errno::EOPNOTSUPP);
    }
    let credentials = kernel.credentials(tid).clone();
    let caller = kernel.threads[&tid].process;
    let value = if !value.is_empty() && name == CAPABILITY {
        convert_capabilities(&credentials, &value)?
    } else {
        value
    };
    let now = kernel.now(cx);
    let backend = on_backend(&node);
    let mut inode = node.borrow_mut();
    permission(&inode, &credentials, caller, &name, true)?;
    // The security layer passes over private inodes (`IS_PRIVATE`): descriptors' inodes.
    if !inode.is_anonymous() {
        security_check(&credentials, &name, false)?;
    }
    resolve_name(&inode, &name)?;
    if backend {
        drop(inode);
        crate::fs::backend::setxattr(&node, &name, &value, flags)?;
        return Ok(0);
    }
    // tmpfs takes the value's space before it looks for the name (`shmem_xattr_handler_set`).
    inode.reserve_xattr(&name, &value)?;
    let exists = inode.xattrs.contains_key(&name);
    let refused = if exists && flags & XATTR_CREATE != 0 {
        Some(Errno::EEXIST)
    } else if !exists && flags & XATTR_REPLACE != 0 {
        Some(Errno::ENODATA)
    } else {
        None
    };
    if let Some(error) = refused {
        inode.unreserve_xattr(&name, &value);
        return Err(error);
    }
    inode.put_xattr(name, value);
    inode.ctime = now;
    Ok(0)
}

fn get(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    at: Target,
    [_, name, value, size, ..]: Args,
) -> Result<u64, Errno> {
    let name = import_name(kernel, cx, tid, name)?;
    let node = target(kernel, cx, tid, at)?;
    let size = size.min(u64::from(XATTR_SIZE_MAX)) as usize;
    let acl = name == ACL_ACCESS || name == ACL_DEFAULT;
    if acl && !on_backend(&node) {
        return Err(Errno::EOPNOTSUPP);
    }
    let found = if acl {
        backend_acl(&node, &name)
    } else {
        let inode = node.borrow();
        let credentials = kernel.credentials(tid);
        permission(
            &inode,
            credentials,
            kernel.threads[&tid].process,
            &name,
            false,
        )?;
        resolve_name(&inode, &name)?;
        if on_backend(&node) {
            drop(inode);
            backend_value(&node, &name, size)
        } else {
            let stored = inode.xattrs.get(&name).ok_or(Errno::ENODATA)?;
            if name == CAPABILITY {
                read_capabilities(stored)
            } else {
                Ok(stored.clone())
            }
        }
    };
    let found = match found {
        Err(Errno::ERANGE) if size >= XATTR_SIZE_MAX as usize => return Err(Errno::E2BIG),
        found => found?,
    };
    if size == 0 {
        return Ok(found.len() as u64);
    }
    if found.len() > size {
        return Err(if size >= XATTR_SIZE_MAX as usize {
            Errno::E2BIG
        } else {
            Errno::ERANGE
        });
    }
    guest(kernel, cx, tid).write(value, &found)?;
    Ok(found.len() as u64)
}

fn list(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    at: Target,
    [_, list, size, ..]: Args,
) -> Result<u64, Errno> {
    let node = target(kernel, cx, tid, at)?;
    let size = size.min(u64::from(XATTR_LIST_MAX)) as usize;
    let names = if on_backend(&node) {
        // The backend lists every name, trusted or not (`fuse_listxattr`).
        match crate::fs::backend::listxattr(&node, size as u32) {
            Err(Errno::ERANGE) if size >= XATTR_LIST_MAX as usize => return Err(Errno::E2BIG),
            names => names?,
        }
    } else {
        let inode = node.borrow();
        let trusted = kernel.credentials(tid).has(CAP_SYS_ADMIN);
        let mut names = Vec::new();
        // tmpfs keeps names in a tree that lists them from the greatest down.
        for name in inode.xattrs.keys().rev() {
            if !trusted && name.starts_with(prefix(XATTR_TRUSTED_PREFIX)) {
                continue;
            }
            names.extend_from_slice(name);
            names.push(0);
        }
        names
    };
    if size == 0 {
        return Ok(names.len() as u64);
    }
    if names.len() > size {
        return Err(if size >= XATTR_LIST_MAX as usize {
            Errno::E2BIG
        } else {
            Errno::ERANGE
        });
    }
    guest(kernel, cx, tid).write(list, &names)?;
    Ok(names.len() as u64)
}

fn remove(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    at: Target,
    [_, name, ..]: Args,
) -> Result<u64, Errno> {
    let name = import_name(kernel, cx, tid, name)?;
    let node = target(kernel, cx, tid, at)?;
    if node.borrow().fs.read_only() {
        return Err(Errno::EROFS);
    }
    if name == ACL_ACCESS || name == ACL_DEFAULT {
        may_write(&node.borrow())?;
        if on_backend(&node) {
            return set_backend_acl(kernel, tid, &node, &name, &[]);
        }
        return Err(Errno::EOPNOTSUPP);
    }
    let credentials = kernel.credentials(tid).clone();
    let caller = kernel.threads[&tid].process;
    let now = kernel.now(cx);
    let backend = on_backend(&node);
    let mut inode = node.borrow_mut();
    permission(&inode, &credentials, caller, &name, true)?;
    // The security layer passes over private inodes (`IS_PRIVATE`): descriptors' inodes.
    if !inode.is_anonymous() {
        security_check(&credentials, &name, true)?;
    }
    resolve_name(&inode, &name)?;
    if backend {
        drop(inode);
        crate::fs::backend::removexattr(&node, &name)?;
        return Ok(0);
    }
    inode.remove_xattr(&name).ok_or(Errno::ENODATA)?;
    inode.ctime = now;
    Ok(0)
}

/// An attribute's value from the backend, at most `size` bytes of it (`__vfs_getxattr`).
fn backend_value(node: &Node, name: &[u8], size: usize) -> Result<Vec<u8>, Errno> {
    if name != CAPABILITY {
        return crate::fs::backend::getxattr(node, name, size as u32);
    }
    // Measured on the reference (7.0): reading file capabilities of a FUSE file is EINVAL, with
    // them set or not and whatever the size; `cap_inode_getsecurity` answers so when it finds no
    // dentry for the inode, the one EINVAL on its way (why it finds none is not known).
    Err(Errno::EINVAL)
}

fn by_path(args: Args, follow: bool) -> Target {
    Target::Path {
        path: args[0],
        follow,
    }
}

pub(super) fn setxattr(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    args: Args,
) -> Result<u64, Errno> {
    set(kernel, cx, tid, by_path(args, true), args)
}

pub(super) fn lsetxattr(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    args: Args,
) -> Result<u64, Errno> {
    set(kernel, cx, tid, by_path(args, false), args)
}

pub(super) fn fsetxattr(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    args: Args,
) -> Result<u64, Errno> {
    set(kernel, cx, tid, Target::File(args[0]), args)
}

pub(super) fn getxattr(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    args: Args,
) -> Result<u64, Errno> {
    get(kernel, cx, tid, by_path(args, true), args)
}

pub(super) fn lgetxattr(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    args: Args,
) -> Result<u64, Errno> {
    get(kernel, cx, tid, by_path(args, false), args)
}

pub(super) fn fgetxattr(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    args: Args,
) -> Result<u64, Errno> {
    get(kernel, cx, tid, Target::File(args[0]), args)
}

pub(super) fn listxattr(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    args: Args,
) -> Result<u64, Errno> {
    list(kernel, cx, tid, by_path(args, true), args)
}

pub(super) fn llistxattr(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    args: Args,
) -> Result<u64, Errno> {
    list(kernel, cx, tid, by_path(args, false), args)
}

pub(super) fn flistxattr(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    args: Args,
) -> Result<u64, Errno> {
    list(kernel, cx, tid, Target::File(args[0]), args)
}

pub(super) fn removexattr(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    args: Args,
) -> Result<u64, Errno> {
    remove(kernel, cx, tid, by_path(args, true), args)
}

pub(super) fn lremovexattr(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    args: Args,
) -> Result<u64, Errno> {
    remove(kernel, cx, tid, by_path(args, false), args)
}

pub(super) fn fremovexattr(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    args: Args,
) -> Result<u64, Errno> {
    remove(kernel, cx, tid, Target::File(args[0]), args)
}
