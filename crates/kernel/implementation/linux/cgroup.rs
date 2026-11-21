//! The container's own cgroup, which Docker mounts read-only at `/sys/fs/cgroup` (cgroup2): the
//! files of its controllers that say what this kernel does, with the values a default container
//! shows in the reference (no limit set); the others are not built, and a lookup of one is counted.

use alloc::string::{String, ToString};

use crate::fs::proc::View;

/// Docker's mount of it.
pub const MOUNT: crate::DockerMount = crate::DockerMount {
    point: "/sys/fs/cgroup",
    source: "cgroup",
    kind: "cgroup2",
    options: "",
};

/// The controllers Docker enables in a container's cgroup, as the reference lists them.
const CONTROLLERS: &str = "cpuset cpu io memory hugetlb pids rdma\n";
/// What ends each line of a file.
const LINE_END: &str = "\n";
/// What a limit that is not set reads.
const UNLIMITED: &str = "max\n";

/// What a file reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Value {
    Text(&'static str),
    /// A setting that is off, a minimum of none, or a count of nothing.
    Zero,
    /// The container's processes, one number a line (`cgroup.procs`).
    Processes,
    /// Their threads (`cgroup.threads`).
    Threads,
}

/// Each file: its name, whether it is a setting (its owner may write it where the mount allows),
/// and what it reads.
pub(crate) const FILES: [(&str, bool, Value); 23] = [
    ("cgroup.controllers", false, Value::Text(CONTROLLERS)),
    (
        "cgroup.events",
        false,
        Value::Text("populated 1\nfrozen 0\n"),
    ),
    ("cgroup.freeze", true, Value::Zero),
    ("cgroup.max.depth", true, Value::Text(UNLIMITED)),
    ("cgroup.max.descendants", true, Value::Text(UNLIMITED)),
    ("cgroup.procs", true, Value::Processes),
    ("cgroup.subtree_control", true, Value::Text("")),
    ("cgroup.threads", true, Value::Threads),
    ("cgroup.type", true, Value::Text("domain\n")),
    ("cpu.idle", true, Value::Zero),
    ("cpu.max", true, Value::Text("max 100000\n")),
    ("cpu.max.burst", true, Value::Zero),
    ("cpu.weight", true, Value::Text("100\n")),
    ("cpu.weight.nice", true, Value::Zero),
    ("cpuset.cpus.partition", true, Value::Text("member\n")),
    ("memory.high", true, Value::Text(UNLIMITED)),
    ("memory.low", true, Value::Zero),
    ("memory.max", true, Value::Text(UNLIMITED)),
    ("memory.min", true, Value::Zero),
    ("memory.oom.group", true, Value::Zero),
    ("memory.swap.high", true, Value::Text(UNLIMITED)),
    ("memory.swap.max", true, Value::Text(UNLIMITED)),
    ("pids.max", true, Value::Text(UNLIMITED)),
];

/// The bytes of the file at `index` in [`FILES`].
pub(crate) fn contents(view: &View<'_>, index: usize) -> String {
    let mut out = String::new();
    let mut line = |value: u64| {
        out.push_str(&value.to_string());
        out.push_str(LINE_END);
    };
    match FILES[index].2 {
        Value::Text(text) => return text.into(),
        Value::Zero => line(u64::MIN),
        Value::Processes => {
            for (pid, _) in view
                .processes
                .iter()
                .filter(|(_, process)| process.ended.is_none())
            {
                line(u64::from(pid.unsigned_abs()));
            }
        }
        Value::Threads => {
            for process in view
                .processes
                .values()
                .filter(|process| process.ended.is_none())
            {
                for tid in crate::fs::proc::tasks(process) {
                    line(u64::from(tid.unsigned_abs()));
                }
            }
        }
    }
    out
}
