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
        &["head", "-c", "0", "/dev/urandom"],
        &["cat", "/proc/self/cmdline"],
        &["readlink", "/proc/self/exe"],
        &["ls", "/proc/self/fd"],
    ]);
}

/// What these show depends on the root file system's own kind: on the reference it is overlayfs over
/// ext4 (directory sizes of 4096, link counts of 1, hash order in listings, 4 KiB blocks per directory),
/// and here, until the file backend exists, it is the in-memory kind.
#[test]
#[ignore = "known failure: the root file system is in memory until the file backend (K4); the reference's is overlayfs over ext4"]
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
