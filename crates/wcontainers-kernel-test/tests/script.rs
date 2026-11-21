//! Scripts that name their interpreter on the first line start as on real ARM64 Linux.

use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use wcontainers_kernel_test::run_both;

#[test]
fn scripts_match_reference() {
    let directory = Path::new(env!("CARGO_TARGET_TMPDIR")).join("scripts");
    std::fs::create_dir_all(&directory).unwrap();
    let scripts = [
        (
            "plain",
            "#!/bin/sh\necho \"$0\" \"$@\"\ncd /dev/shm && pwd\nexit 7\n",
        ),
        (
            "argument",
            "#! /bin/sh -e\necho with an argument \"$1\"\nfalse\necho not reached\n",
        ),
        ("missing", "#!/nonexistent/interpreter\necho never\n"),
        ("busybox", "#!/bin/busybox echo\n"),
        // A zero ends the interpreter's name, and its argument.
        ("zero_after_name", "#!/bin/echo\0junk here\n"),
        ("zero_in_argument", "#!/bin/echo a\0b\n"),
        ("zero_after_blank", "#!/bin/echo  \0arg\n"),
        ("blanks_around", "#! \t/bin/echo \t one  two \t\n"),
    ];
    let mut failed = Vec::new();
    for (name, text) in scripts {
        let path = directory.join(name);
        std::fs::write(&path, text).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        if name == "missing" {
            // Docker itself reports a script it cannot start; only our kernel's answer is checked.
            let ours = wcontainers_kernel_test::driver::run(
                &wcontainers_kernel_test::driver::Container {
                    files: &{
                        let mut files = wcontainers_kernel_test::image::reference_image().to_vec();
                        files.extend(wcontainers_kernel_test::image::host_directory(
                            &directory, "wc",
                        ));
                        files
                    },
                    path: "/wc/missing",
                    arguments: &["/wc/missing"],
                    environment: wcontainers_kernel_test::reference::ENVIRONMENT,
                    working_directory: "/",
                    backend: None,
                    cached: false,
                    published: &[],
                    root_image: None,
                    address_space: wcontainers_kernel_test::standin::SPACE_32_BIT,
                },
                0,
            );
            assert_eq!(
                ours.err(),
                Some(wcontainers_kernel_test::driver::Failure::Start(
                    wcontainers_kernel_linux::Errno(2)
                ))
            );
            continue;
        }
        let (ours, theirs) = run_both(&path, &["one", "two"]);
        if ours.outcome != theirs {
            failed.push(format!(
                "{name}: ours {:?}, theirs {:?}",
                ours.outcome, theirs
            ));
        }
    }
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}
