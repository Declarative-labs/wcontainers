//! The init of a small virtual machine: sets an overcommit mode, then in a child process maps memory with
//! `HostMapping` and touches every page through `SharedMemory`, and reports how the child ended.
use std::fs;
use std::io::Write;
use std::os::fd::AsFd;
use std::os::unix::process::ExitStatusExt;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use rustix::fs::{MemfdFlags, SealFlags, fcntl_add_seals, ftruncate, memfd_create};
use wcontainers_shared_memory::{HostMapping, SharedMemory};

fn meminfo(key: &str) -> u64 {
    fs::read_to_string("/proc/meminfo")
        .unwrap()
        .lines()
        .find(|l| l.starts_with(key))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
        * 1024
}

fn touch(memory: &HostMapping, label: &str) {
    let page = rustix::param::page_size() as u64;
    let mut out = std::io::stdout();
    let total = memory.len() / page;
    for p in 0..total {
        memory.store(p * page, p + 1).unwrap();
        if p % (total / 8).max(1) == 0 {
            writeln!(
                out,
                "  {label}: page {p} of {total} written, Committed_AS {} MiB",
                meminfo("Committed_AS:") >> 20
            )
            .ok();
        }
    }
    writeln!(
        out,
        "  {label}: all {total} pages written, every load {}",
        (0..total).all(|p| memory.load::<u64>(p * page) == Ok(p + 1))
    )
    .ok();
}

fn child(case: &str, bytes: u64) {
    let len = bytes as usize;
    match case {
        "file" => {
            let file =
                memfd_create("probe", MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING).unwrap();
            ftruncate(&file, bytes).unwrap();
            fcntl_add_seals(&file, SealFlags::SHRINK).unwrap();
            match HostMapping::file(file.as_fd(), 0, len) {
                Ok(m) => {
                    println!("  file: mapped {} MiB", bytes >> 20);
                    touch(&m, "file");
                }
                Err(e) => println!("  file: refused: {e}"),
            }
        }
        _ => match HostMapping::anonymous(len) {
            Ok(m) => {
                println!(
                    "  anonymous: mapped {} MiB, Committed_AS {} MiB",
                    bytes >> 20,
                    meminfo("Committed_AS:") >> 20
                );
                touch(&m, "anonymous");
            }
            Err(e) => println!("  anonymous: refused: {e}"),
        },
    }
}

fn run(mode: &str, case: &str, bytes: u64) {
    fs::write("/proc/sys/vm/overcommit_memory", mode).unwrap();
    println!(
        "== overcommit_memory {mode}, {case}, {} MiB: MemTotal {} MiB, CommitLimit {} MiB, Committed_AS {} MiB",
        bytes >> 20,
        meminfo("MemTotal:") >> 20,
        meminfo("CommitLimit:") >> 20,
        meminfo("Committed_AS:") >> 20
    );
    let mut c = Command::new("/init")
        .arg(case)
        .arg(bytes.to_string())
        .stdout(Stdio::inherit())
        .spawn()
        .unwrap();
    let start = Instant::now();
    loop {
        if let Some(status) = c.try_wait().unwrap() {
            println!(
                "  ended after {:?}: exit {:?}, signal {:?}",
                start.elapsed(),
                status.code(),
                status.signal()
            );
            break;
        }
        if start.elapsed() > Duration::from_secs(30) {
            println!("  still running after 30 s: hung; killing it");
            c.kill().ok();
            c.wait().ok();
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    fs::write("/proc/sys/vm/overcommit_memory", "0").unwrap();
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() == 3 {
        child(&args[1], args[2].parse().unwrap());
        return;
    }
    fs::create_dir_all("/proc").ok();
    rustix::mount::mount(
        "proc",
        "/proc",
        "proc",
        rustix::mount::MountFlags::empty(),
        None,
    )
    .unwrap();
    println!(
        "probe: kernel {}",
        fs::read_to_string("/proc/sys/kernel/osrelease")
            .unwrap()
            .trim()
    );
    let total = meminfo("MemTotal:");
    let mib = 1 << 20;
    let big = total * 8 / 10 / mib * mib;
    for mode in ["2", "0"] {
        run(mode, "file", big);
        run(mode, "anonymous", big);
    }
    println!("probe: done");
    std::io::stdout().flush().ok();
    rustix::system::reboot(rustix::system::RebootCommand::PowerOff).ok();
}
