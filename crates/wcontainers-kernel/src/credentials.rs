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

#[derive(Clone, Debug)]
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

    pub fn in_group(&self, gid: u32, use_fs_ids: bool) -> bool {
        let own = if use_fs_ids { self.fsgid } else { self.egid };
        own == gid || self.groups.binary_search(&gid).is_ok()
    }

    /// Linux's `cap_emulate_setxuid`: what a change of user numbers does to the powers.
    pub fn after_uid_change(&mut self, old_uid: u32, old_euid: u32, old_suid: u32) {
        let was_root = old_uid == 0 || old_euid == 0 || old_suid == 0;
        let all_nonroot = self.uid != 0 && self.euid != 0 && self.suid != 0;
        if was_root && all_nonroot && !self.keep_capabilities {
            self.permitted = 0;
            self.effective = 0;
            self.ambient = 0;
        }
        if old_euid == 0 && self.euid != 0 {
            self.effective = 0;
        }
        if old_euid != 0 && self.euid == 0 {
            self.effective = self.permitted;
        }
        // The file system user follows the effective one, and its powers follow it too.
        let old_fsuid = self.fsuid;
        self.fsuid = self.euid;
        self.after_fsuid_change(old_fsuid);
    }

    /// Linux's `cap_emulate_setxuid` for the file system user: leaving root drops the file powers.
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
