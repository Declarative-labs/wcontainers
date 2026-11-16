//! Who a process is: user and group numbers and its powers (capabilities).

use alloc::vec::Vec;

use crate::linux::general;

pub(crate) const CAP_CHOWN: u32 = general::CAP_CHOWN;
pub(crate) const CAP_DAC_OVERRIDE: u32 = general::CAP_DAC_OVERRIDE;
pub(crate) const CAP_DAC_READ_SEARCH: u32 = general::CAP_DAC_READ_SEARCH;
pub(crate) const CAP_FOWNER: u32 = general::CAP_FOWNER;
pub(crate) const CAP_FSETID: u32 = general::CAP_FSETID;
pub(crate) const CAP_SETGID: u32 = general::CAP_SETGID;
pub(crate) const CAP_SETUID: u32 = general::CAP_SETUID;
pub(crate) const CAP_MKNOD: u32 = general::CAP_MKNOD;
pub(crate) const CAP_SYS_CHROOT: u32 = general::CAP_SYS_CHROOT;
pub(crate) const CAP_SYS_ADMIN: u32 = general::CAP_SYS_ADMIN;
pub(crate) const CAP_SYS_RAWIO: u32 = general::CAP_SYS_RAWIO;
pub(crate) const CAP_LINUX_IMMUTABLE: u32 = general::CAP_LINUX_IMMUTABLE;
pub(crate) const CAP_SETFCAP: u32 = general::CAP_SETFCAP;

/// The powers a default Docker container gives its processes.
pub(crate) const DOCKER_DEFAULT: u64 = bits(&[
    general::CAP_CHOWN,
    general::CAP_DAC_OVERRIDE,
    general::CAP_FOWNER,
    general::CAP_FSETID,
    general::CAP_KILL,
    general::CAP_SETGID,
    general::CAP_SETUID,
    general::CAP_SETPCAP,
    general::CAP_NET_BIND_SERVICE,
    general::CAP_NET_RAW,
    general::CAP_SYS_CHROOT,
    general::CAP_MKNOD,
    general::CAP_AUDIT_WRITE,
    general::CAP_SETFCAP,
]);

const fn bits(capabilities: &[u32]) -> u64 {
    let mut set = 0;
    let mut index = 0;
    while index < capabilities.len() {
        set |= 1 << capabilities[index];
        index += 1;
    }
    set
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Credentials {
    pub uid: u32,
    pub euid: u32,
    pub suid: u32,
    pub fsuid: u32,
    pub gid: u32,
    pub egid: u32,
    pub sgid: u32,
    pub fsgid: u32,
    /// Supplementary groups, sorted as Linux keeps them.
    pub groups: Vec<u32>,
    pub effective: u64,
    pub permitted: u64,
    pub inheritable: u64,
    pub bounding: u64,
    pub ambient: u64,
    /// `PR_SET_KEEPCAPS`.
    pub keep_capabilities: bool,
    /// `PR_SET_NO_NEW_PRIVS`.
    pub no_new_privileges: bool,
    /// Which committed change of credentials these are: Linux makes a new credential structure at
    /// each `commit_creds`, and some checks compare an open file's with the caller's by identity.
    pub serial: u64,
}

impl Credentials {
    /// Root with Docker's default powers and the given supplementary groups.
    pub fn container_root(mut groups: Vec<u32>) -> Self {
        groups.sort_unstable();
        Self {
            uid: 0,
            euid: 0,
            suid: 0,
            fsuid: 0,
            gid: 0,
            egid: 0,
            sgid: 0,
            fsgid: 0,
            groups,
            effective: DOCKER_DEFAULT,
            permitted: DOCKER_DEFAULT,
            inheritable: 0,
            bounding: DOCKER_DEFAULT,
            ambient: 0,
            keep_capabilities: false,
            no_new_privileges: false,
            serial: 0,
        }
    }

    /// Whether a change from `old` makes the process undumpable, as Linux's `commit_creds` decides: a
    /// change of effective or file system ids, or powers gained.
    pub fn loses_dumpable(&self, old: &Credentials) -> bool {
        self.euid != old.euid
            || self.egid != old.egid
            || self.fsuid != old.fsuid
            || self.fsgid != old.fsgid
            || self.permitted & !old.permitted != 0
    }

    pub fn has(&self, capability: u32) -> bool {
        self.effective & (1 << capability) != 0
    }

    /// Linux's `inode_owner_or_capable`: the file system user owns a file of owner `uid`, or holds
    /// CAP_FOWNER.
    pub fn owns_or_capable(&self, uid: u32) -> bool {
        self.fsuid == uid || self.has(general::CAP_FOWNER)
    }

    pub fn in_group(&self, gid: u32, use_fs_ids: bool) -> bool {
        let own = if use_fs_ids { self.fsgid } else { self.egid };
        own == gid || self.groups.binary_search(&gid).is_ok()
    }

    /// Linux's `cap_emulate_setxuid`: what a change of user numbers does to the powers.
    pub fn after_uid_change(&mut self, old_uid: u32, old_euid: u32, old_suid: u32) {
        let was_root = old_uid == 0 || old_euid == 0 || old_suid == 0;
        let all_nonroot = self.uid != 0 && self.euid != 0 && self.suid != 0;
        if was_root && all_nonroot {
            if !self.keep_capabilities {
                self.permitted = 0;
                self.effective = 0;
            }
            self.ambient = 0;
        }
        if old_euid == 0 && self.euid != 0 {
            self.effective = 0;
        }
        if old_euid != 0 && self.euid == 0 {
            self.effective = self.permitted;
        }
        // The file system user follows the effective one; only `setfsuid` moves the file powers with
        // it (`cap_task_fix_setuid`).
        self.fsuid = self.euid;
    }

    /// Linux's `cap_task_fix_setuid` for `setfsuid`: leaving root drops the file powers.
    pub fn after_fsuid_change(&mut self, old_fsuid: u32) {
        const FILE_POWERS: u64 = bits(&[
            general::CAP_CHOWN,
            general::CAP_DAC_OVERRIDE,
            general::CAP_DAC_READ_SEARCH,
            general::CAP_FOWNER,
            general::CAP_FSETID,
            general::CAP_LINUX_IMMUTABLE,
            general::CAP_MKNOD,
            general::CAP_MAC_OVERRIDE,
        ]);
        if old_fsuid == 0 && self.fsuid != 0 {
            self.effective &= !FILE_POWERS;
        }
        if old_fsuid != 0 && self.fsuid == 0 {
            self.effective |= self.permitted & FILE_POWERS;
        }
    }
}

/// What the file a program starts from says about the credentials it runs with.
pub(crate) struct ProgramFile<'a> {
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
    /// It is on a mount that ignores set-user bits and file capabilities (`nosuid`).
    pub nosuid: bool,
    /// Its stored file capabilities, if it has any.
    pub capabilities: Option<&'a [u8]>,
}

/// What `execve` does to a process's identity.
pub(crate) struct AfterExec {
    pub credentials: Credentials,
    /// The program runs in secure mode (`AT_SECURE`).
    pub secure: bool,
    /// The program gained privileges, which clears some `personality` bits.
    pub gained: bool,
}

/// The powers Linux knows (`CAP_VALID_MASK`).
const VALID: u64 = (1 << (general::CAP_LAST_CAP + 1)) - 1;

/// File capabilities as Linux's `get_vfs_caps_from_disk` reads them: permitted, inheritable and whether
/// they are made effective. None where Linux finds none (owned by a user other than root).
fn file_capabilities(stored: &[u8]) -> Result<Option<(u64, u64, bool)>, crate::Errno> {
    use crate::Errno;
    use general::{VFS_CAP_REVISION_2, VFS_CAP_REVISION_3, VFS_CAP_REVISION_MASK};
    if stored.len() > size_of::<general::vfs_ns_cap_data>() {
        return Err(Errno::ERANGE);
    }
    if stored.len() < 4 {
        return Err(Errno::EINVAL);
    }
    let word = |index: usize| {
        u64::from(u32::from_le_bytes(
            stored[index * 4..index * 4 + 4]
                .try_into()
                .expect("four bytes"),
        ))
    };
    let magic = word(0) as u32;
    let root = match magic & VFS_CAP_REVISION_MASK {
        VFS_CAP_REVISION_2 if stored.len() == size_of::<general::vfs_cap_data>() => 0,
        VFS_CAP_REVISION_3 if stored.len() == size_of::<general::vfs_ns_cap_data>() => word(5),
        _ => return Err(Errno::EINVAL),
    };
    if root != 0 {
        return Ok(None);
    }
    let permitted = (word(1) | word(3) << 32) & VALID;
    let inheritable = (word(2) | word(4) << 32) & VALID;
    Ok(Some((
        permitted,
        inheritable,
        magic & general::VFS_CAP_FLAGS_EFFECTIVE != 0,
    )))
}

impl Credentials {
    /// The credentials a program runs with, as Linux's `bprm_fill_uid` and `cap_bprm_creds_from_file`
    /// work them out for a process nobody traces: set-user and set-group bits, file capabilities and
    /// root's powers. An error refuses the `execve`.
    pub fn for_program(&self, file: &ProgramFile<'_>) -> Result<AfterExec, crate::Errno> {
        let old = self;
        let mut new = self.clone();
        let mut gained = false;
        if !file.nosuid && !old.no_new_privileges {
            if file.mode & general::S_ISUID != 0 {
                new.euid = file.uid;
                gained = true;
            }
            if file.mode & (general::S_ISGID | general::S_IXGRP)
                == general::S_ISGID | general::S_IXGRP
            {
                new.egid = file.gid;
                gained = true;
            }
        }
        new.permitted = 0;
        let mut effective = false;
        let mut has_file_capabilities = false;
        if !file.nosuid
            && let Some(stored) = file.capabilities
            && let Some((permitted, inheritable, raise)) = file_capabilities(stored)?
        {
            effective = raise;
            has_file_capabilities = true;
            new.permitted = (new.bounding & permitted) | (new.inheritable & inheritable);
            if permitted & !new.permitted != 0 {
                return Err(crate::Errno::EPERM);
            }
        }
        // Root's powers, unless a set-user-root program with file capabilities is run by another user.
        let set_user_root = new.uid != 0 && new.euid == 0;
        if !(has_file_capabilities && set_user_root) {
            if new.euid == 0 || new.uid == 0 {
                new.permitted = new.bounding | new.inheritable;
            }
            if new.euid == 0 {
                effective = true;
            }
        }
        let capabilities_gained = new.permitted & !old.permitted != 0;
        gained |= capabilities_gained;
        let id_changed = new.euid != old.euid || !old.in_group(new.egid, true);
        if (id_changed || capabilities_gained) && old.no_new_privileges {
            new.euid = new.uid;
            new.egid = new.gid;
            new.permitted &= old.permitted;
        }
        new.suid = new.euid;
        new.fsuid = new.euid;
        new.sgid = new.egid;
        new.fsgid = new.egid;
        if has_file_capabilities || id_changed {
            new.ambient = 0;
        }
        new.permitted |= new.ambient;
        new.effective = if effective {
            new.permitted
        } else {
            new.ambient
        };
        new.keep_capabilities = false;
        let secure = id_changed
            || new.euid != old.uid
            || new.egid != old.gid
            || (new.uid != 0 && (effective || new.permitted & !new.ambient != 0));
        Ok(AfterExec {
            credentials: new,
            secure,
            gained,
        })
    }
}
