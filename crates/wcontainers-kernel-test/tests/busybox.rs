//! Single programs from busybox in the reference image run and match the reference in output, exit
//! status and the files they leave behind.

use wcontainers_kernel_test::busybox::{FILES, TEXT, compare, each};

/// Runs each command in both runners and compares, reporting every command that differs.
fn all(commands: &[&[&str]]) {
    each(commands, |command| {
        compare(command);
    });
}

#[test]
fn text() {
    all(TEXT);
}

#[test]
fn files() {
    all(FILES);
}

#[test]
fn system() {
    all(&[
        &["id"],
        &["whoami"],
        &["hostname"],
        &["env"],
        &["sleep", "1"],
        &["getconf", "PAGE_SIZE"],
        &["ls", "-l", "/dev"],
        &["find", "/dev"],
        &["ls", "-f", "/dev"],
        &["head", "-c", "0", "/dev/urandom"],
        &["cat", "/proc/self/cmdline"],
        &["readlink", "/proc/self/exe"],
        &["ls", "/proc/self/fd"],
        &[
            "sh",
            "-c",
            "echo a; cd /dev/shm; pwd; umask 077; umask; exit 3",
        ],
        &["sh", "-c", "exec ls -l /dev/null"],
        &[
            "sh",
            "-c",
            "zcat /proc/config.gz | grep -E \
             '^CONFIG_(IKCONFIG_PROC|DNOTIFY|USER_NS|SECCOMP_FILTER|TMPFS_XATTR|CHECKPOINT_RESTORE)='",
        ],
        &[
            "sh",
            "-c",
            "cd /proc/sys; for f in fs/dir-notify-enable fs/mqueue/* fs/pipe-max-size \
             kernel/msgmax kernel/msgmnb kernel/msgmni kernel/sem kernel/shmall kernel/shmmax \
             kernel/shmmni kernel/random/entropy_avail kernel/random/poolsize \
             fs/inotify/max_queued_events fs/inotify/max_user_instances \
             fs/inotify/max_user_watches fs/protected_* kernel/domainname kernel/core_pattern \
             kernel/core_uses_pid kernel/core_pipe_limit kernel/core_sort_vma \
             kernel/core_file_note_size_limit \
             kernel/io_uring_disabled kernel/perf_event_paranoid vm/nr_hugepages \
             vm/nr_overcommit_hugepages vm/hugetlb_shm_group; \
             do echo $f $(stat -c %a $f) $(cat $f); done; cat vm/drop_caches; \
             echo threads-max $(stat -c %a kernel/threads-max) $(test $(cat kernel/threads-max) -gt 19 && echo set); \
             test -s kernel/ns_last_pid && echo has ns_last_pid; echo 1 > kernel/shmmni; echo $?",
        ],
        &[
            "sh",
            "-c",
            "read x < /etc/hostname; echo \"$x\"; test -d /proc && echo yes",
        ],
        &["nice", "-n", "5", "nice"],
        &["setpriv", "--dump"],
        &[
            "setpriv",
            "--inh-caps=+chown",
            "--ambient-caps=+chown",
            "setpriv",
            "--dump",
        ],
        &["renice", "-n", "3", "-p", "1"],
        &[
            "setpriv",
            "--reuid=1000",
            "--regid=1000",
            "--clear-groups",
            "id",
        ],
        &["linux32", "uname", "-m"],
        &["linux64", "uname", "-m"],
        &["sysctl", "kernel.hostname"],
        &["sysctl", "-w", "kernel.hostname=x"],
        &["pwdx", "1"],
        &["ionice", "-p", "1"],
        &["ionice", "-c", "1", "-p", "1"],
        &["ionice", "-c", "2", "-n", "5", "ionice", "-p", "1"],
        &["ionice", "-c", "3", "ionice", "-p", "0"],
        &["cat", "/proc/self/limits"],
        &["cat", "/proc/self/comm"],
        &[
            "sh",
            "-c",
            "echo hi > /dev/shm/f; read y < /dev/shm/f; echo $y >> /dev/shm/f",
        ],
    ]);
}

/// What these show depends on the root file system's own kind: on the reference it is overlayfs over
/// ext4 (directory sizes of 4096, link counts of 1, hash order in listings, 4 KiB blocks per directory),
/// and here it is the in-memory kind, or on the backend the kind of the server's directory (tmpfs
/// in tests).
#[test]
#[ignore = "known failure: the reference's root is overlayfs over ext4; ours is in memory or on the test backend's tmpfs, which show other sizes, link counts and listing order"]
fn root_file_system() {
    all(&[
        &["ls", "-la", "/"],
        &["ls", "-ln", "/etc"],
        &["stat", "-c", "%n %s %f %u %g %h", "/etc"],
        &["find", "/etc", "-name", "*.conf"],
        &["du", "-s", "/etc/apk"],
    ]);
}

/// The guest is told it has one CPU (section 15 of the plan); the reference machine has several.
#[test]
fn one_cpu() {
    let ours = wcontainers_kernel_test::busybox::run_ours(&["nproc"]);
    assert_eq!(ours.outcome.stdout, b"1\n");
}
