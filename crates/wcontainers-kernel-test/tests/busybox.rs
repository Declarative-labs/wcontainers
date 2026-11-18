//! Single programs from busybox in the reference image run and match the reference in output, exit
//! status and the files they leave behind.

use wcontainers_kernel_test::busybox::compare;

/// Runs each command in both runners and compares, reporting every command that differs.
fn all(commands: &[&[&str]]) {
    let mut failed = Vec::new();
    for command in commands {
        let result = std::panic::catch_unwind(|| compare(command));
        if let Err(error) = result {
            let message = error
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| error.downcast_ref::<&str>().map(|text| text.to_string()))
                .unwrap_or_default();
            failed.push(format!("{command:?}: {message}"));
        }
    }
    assert!(
        failed.is_empty(),
        "{} of {} differ:\n{}",
        failed.len(),
        commands.len(),
        failed.join("\n\n")
    );
}

#[test]
fn text() {
    all(&[
        &["echo", "hello", "world"],
        &["printf", "%s-%d\\n", "a", "42"],
        &["cat", "/etc/passwd"],
        &["cat", "/nonexistent"],
        &["cat", "/etc/hostname/"],
        &["cat", "/dev/null/"],
        &["head", "-n", "3", "/etc/group"],
        &["tail", "-c", "20", "/etc/os-release"],
        &["wc", "/etc/passwd"],
        &["sort", "-r", "/etc/shells"],
        &["grep", "-n", "root", "/etc/passwd"],
        &["grep", "nothing-matches", "/etc/passwd"],
        &["sed", "-n", "2,3p", "/etc/passwd"],
        &["awk", "-F:", "{print $1}", "/etc/passwd"],
        &["cut", "-d:", "-f1,3", "/etc/group"],
        &["tr", "a-z", "A-Z"],
        &["seq", "1", "5"],
        &["md5sum", "/bin/busybox"],
        &["sha256sum", "/etc/passwd"],
        &["base64", "/etc/hostname"],
        &["od", "-An", "-tx1", "-N", "16", "/bin/busybox"],
        &["expr", "6", "*", "7"],
        &["basename", "/a/b/c.txt", ".txt"],
        &["true"],
        &["false"],
    ]);
}

#[test]
fn files() {
    all(&[
        &["ls", "-R", "/etc/apk"],
        &["mkdir", "-p", "/tmp/a/b/c"],
        &["mkdir", "/etc/passwd"],
        &["touch", "-d", "2020-01-02 03:04:05", "/tmp/new"],
        &["rm", "/etc/motd"],
        &["rm", "-r", "/etc/apk"],
        &["rmdir", "/home"],
        &["mv", "/etc/motd", "/tmp/motd"],
        &["cp", "-a", "/etc/apk", "/tmp/apk"],
        &[
            "stat",
            "-c",
            "%n %s %f %u %g %h",
            "/etc/passwd",
            "/bin/sh",
            "/dev/shm",
            "/dev/null",
        ],
        &["ls", "-la", "/dev/shm", "/dev/pts"],
        &["ln", "-s", "/etc/passwd", "/tmp/link"],
        &["ln", "/etc/passwd", "/tmp/hard"],
        &["chmod", "4755", "/etc/motd"],
        &["chown", "1:2", "/etc/motd"],
        &["readlink", "-f", "/bin/sh"],
        &["realpath", "/usr/../etc/./passwd"],
        &[
            "dd",
            "if=/dev/zero",
            "of=/tmp/zero",
            "bs=1024",
            "count=8",
            "status=noxfer",
        ],
        &["tee", "/tmp/out"],
        &["truncate", "-s", "10000", "/tmp/sparse"],
        &["mknod", "/tmp/null", "c", "1", "3"],
        &["mkfifo", "/tmp/fifo"],
        &["test", "-w", "/etc/passwd"],
        &["pwd"],
        &["fallocate", "-l", "8192", "/tmp/f"],
        &["install", "-m", "755", "/etc/passwd", "/tmp/p"],
        &["readahead", "/etc/passwd"],
        &["fsync", "/etc/passwd"],
        &["cmp", "/bin/busybox", "/bin/sh"],
        &["xxd", "-l", "16", "/bin/busybox"],
    ]);
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
             fs/inotify/max_queued_events fs/protected_* kernel/domainname \
             kernel/io_uring_disabled kernel/perf_event_paranoid vm/nr_hugepages \
             vm/nr_overcommit_hugepages vm/hugetlb_shm_group; \
             do echo $f $(stat -c %a $f) $(cat $f); done; cat vm/drop_caches; \
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
