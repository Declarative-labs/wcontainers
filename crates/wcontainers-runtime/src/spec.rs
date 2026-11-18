//! Every field of the OCI runtime configuration honoured or refused by name. The configuration is read
//! as its JSON, so a field this runtime does not know, oci-spec's newer ones included, is refused
//! too, never passed over.

use oci_spec::runtime::Spec;
use serde_json::{Value, json};

use crate::Error;

/// The powers the kernel gives the first process: a default Docker container's.
const CAPABILITIES: [&str; 14] = [
    "CAP_CHOWN",
    "CAP_DAC_OVERRIDE",
    "CAP_FSETID",
    "CAP_FOWNER",
    "CAP_MKNOD",
    "CAP_NET_RAW",
    "CAP_SETGID",
    "CAP_SETUID",
    "CAP_SETFCAP",
    "CAP_SETPCAP",
    "CAP_NET_BIND_SERVICE",
    "CAP_SYS_CHROOT",
    "CAP_KILL",
    "CAP_AUDIT_WRITE",
];

/// The limits the kernel starts the first process with, as (name, soft, hard); every other limit is
/// unlimited.
const LIMITS: [(&str, u64, u64); 7] = [
    ("RLIMIT_STACK", 8 << 20, u64::MAX),
    ("RLIMIT_CORE", 0, u64::MAX),
    ("RLIMIT_NOFILE", 1 << 20, 1 << 20),
    ("RLIMIT_NICE", 0, 0),
    ("RLIMIT_RTPRIO", 0, 0),
    ("RLIMIT_SIGPENDING", 128_302, 128_302),
    ("RLIMIT_MSGQUEUE", 819_200, 819_200),
];

/// The namespaces every container has its own of, by construction.
const NAMESPACES: [&str; 6] = ["pid", "network", "ipc", "uts", "mount", "cgroup"];

/// The umask the kernel starts the first process with.
const UMASK: u64 = 0o022;

/// The mounts the kernel always makes, as a default Docker container has them.
fn mounts() -> Value {
    json!([
        {"destination": "/proc", "type": "proc", "source": "proc",
         "options": ["nosuid", "noexec", "nodev"]},
        {"destination": "/dev", "type": "tmpfs", "source": "tmpfs",
         "options": ["nosuid", "strictatime", "mode=755", "size=65536k"]},
        {"destination": "/dev/pts", "type": "devpts", "source": "devpts",
         "options": ["nosuid", "noexec", "newinstance", "ptmxmode=0666", "mode=0620", "gid=5"]},
        {"destination": "/sys", "type": "sysfs", "source": "sysfs",
         "options": ["nosuid", "noexec", "nodev", "ro"]},
        {"destination": "/dev/mqueue", "type": "mqueue", "source": "mqueue",
         "options": ["nosuid", "noexec", "nodev"]},
        {"destination": "/dev/shm", "type": "tmpfs", "source": "shm",
         "options": ["nosuid", "noexec", "nodev", "mode=1777", "size=65536k"]},
    ])
}

/// The configuration of everything the kernel gives a container, with no program: what an image's
/// configuration is converted into, and where a configuration for a plain tar starts.
pub fn base_spec() -> Spec {
    let capabilities = json!(CAPABILITIES);
    let namespaces: Vec<Value> = NAMESPACES
        .iter()
        .map(|kind| json!({"type": kind}))
        .collect();
    serde_json::from_value(json!({
        "ociVersion": "1.2.1",
        "root": {"path": "rootfs"},
        "process": {
            "user": {"uid": 0, "gid": 0},
            "cwd": "/",
            "env": ["PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"],
            "capabilities": {
                "bounding": capabilities,
                "effective": capabilities,
                "permitted": capabilities,
            },
        },
        "mounts": mounts(),
        "linux": {"namespaces": namespaces},
    }))
    .expect("the base configuration is one oci-spec reads")
}

/// What the kernel's start takes from an accepted configuration.
pub(crate) struct Accepted {
    pub args: Vec<String>,
    pub env: Vec<String>,
    pub cwd: String,
    pub hostname: String,
    pub groups: Vec<u32>,
}

fn refused(field: impl Into<String>, reason: impl Into<String>) -> Error {
    Error::Refused {
        field: field.into(),
        reason: reason.into(),
    }
}

/// Whether a value says nothing: absent, null, false, or an empty list, map or string.
fn empty(value: &Value) -> bool {
    match value {
        Value::Null | Value::Bool(false) => true,
        Value::Array(items) => items.is_empty(),
        Value::Object(fields) => fields.is_empty(),
        Value::String(text) => text.is_empty(),
        _ => false,
    }
}

/// Refuses `value` at `field` unless it says nothing, with what supporting it would take.
fn absent(field: &str, value: &Value, takes: &str) -> Result<(), Error> {
    if empty(value) {
        Ok(())
    } else {
        Err(refused(field, format!("{value}: {takes}")))
    }
}

/// Checks every field of `spec`; what the kernel's start takes from it.
pub(crate) fn accept(spec: &Spec) -> Result<Accepted, Error> {
    let value = serde_json::to_value(spec).expect("a configuration is JSON");
    let Value::Object(fields) = &value else {
        return Err(refused("", "the configuration is not a JSON object"));
    };
    for (name, field) in fields {
        match name.as_str() {
            "ociVersion" => {
                if !field
                    .as_str()
                    .is_some_and(|version| version.starts_with("1."))
                {
                    return Err(refused(
                        name,
                        format!("{field}: this runtime reads version 1"),
                    ));
                }
            }
            "root" => root(field)?,
            "mounts" => {}
            "process" => process(field)?,
            "hostname" | "annotations" => {}
            "domainname" => absent(name, field, "the kernel taking a domain name at start")?,
            "hooks" => absent(
                name,
                field,
                "the shipped outside running host programs at the points the runtime \
                 specification names",
            )?,
            "linux" => linux(field)?,
            "uidMappings" | "gidMappings" => absent(name, field, "user namespaces in the kernel")?,
            "solaris" | "windows" | "vm" | "zos" => {
                absent(name, field, "another kind of host than Linux")?;
            }
            _ => return Err(refused(name, "a field this runtime does not know")),
        }
    }
    // The kernel makes its mounts and namespaces whatever the configuration says, so leaving the lists
    // out is refused as leaving one of them out is.
    check_mounts(fields.get("mounts").unwrap_or(&Value::Null))?;
    if !fields.contains_key("linux") {
        namespaces(&Value::Null)?;
    }
    let process = spec
        .process()
        .as_ref()
        .ok_or_else(|| refused("process", "a container needs a process to run"))?;
    let args = process.args().clone().unwrap_or_default();
    if args.is_empty() {
        return Err(refused(
            "process.args",
            "a container needs a program to run",
        ));
    }
    let cwd = process.cwd().to_string_lossy().into_owned();
    if !cwd.starts_with('/') {
        return Err(refused(
            "process.cwd",
            format!("{cwd}: not an absolute path"),
        ));
    }
    Ok(Accepted {
        args,
        env: process.env().clone().unwrap_or_default(),
        cwd,
        hostname: spec
            .hostname()
            .clone()
            .unwrap_or_else(|| "localhost".to_string()),
        groups: process.user().additional_gids().clone().unwrap_or_default(),
    })
}

fn root(value: &Value) -> Result<(), Error> {
    for (name, field) in value.as_object().into_iter().flatten() {
        match name.as_str() {
            "path" => {}
            "readonly" => absent("root.readonly", field, "read-only mounts in the kernel")?,
            _ => {
                return Err(refused(
                    format!("root.{name}"),
                    "a field this runtime does not know",
                ));
            }
        }
    }
    Ok(())
}

/// The mounts must be the kernel's own, each as it makes it: it makes all of them, and no others.
fn check_mounts(value: &Value) -> Result<(), Error> {
    let kernel = mounts();
    let kernel = kernel.as_array().expect("a list");
    let given = value.as_array().map(Vec::as_slice).unwrap_or_default();
    let options = |mount: &Value| -> Vec<String> {
        let mut options: Vec<String> = mount["options"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|option| option.as_str().map(str::to_string))
            .collect();
        options.sort();
        options
    };
    for (index, mount) in given.iter().enumerate() {
        let field = format!("mounts[{index}]");
        if let Some(name) = mount
            .as_object()
            .into_iter()
            .flatten()
            .map(|(name, _)| name)
            .find(|name| !["destination", "type", "source", "options"].contains(&name.as_str()))
        {
            return Err(refused(
                format!("{field}.{name}"),
                "id-mapped mounts: user namespaces in the kernel",
            ));
        }
        let same = kernel.iter().find(|ours| {
            ours["destination"] == mount["destination"]
                && ours["type"] == mount["type"]
                && ours["source"] == mount["source"]
                && options(ours) == options(mount)
        });
        if same.is_none() {
            return Err(refused(
                field,
                format!(
                    "{mount}: only the kernel's own mounts are made; bind mounts of host \
                     directories need the shipped outside's file backend, other file systems the \
                     kernel"
                ),
            ));
        }
    }
    for ours in kernel {
        if !given
            .iter()
            .any(|mount| mount["destination"] == ours["destination"])
        {
            return Err(refused(
                "mounts",
                format!(
                    "lacks {}, which the kernel always mounts; leaving it out needs the kernel to \
                     take its mounts at start",
                    ours["destination"]
                ),
            ));
        }
    }
    Ok(())
}

fn process(value: &Value) -> Result<(), Error> {
    for (name, field) in value.as_object().into_iter().flatten() {
        let path = format!("process.{name}");
        match name.as_str() {
            "args" | "env" | "cwd" => {}
            "terminal" => absent(
                &path,
                field,
                "a terminal: standard input in the kernel's interface and the terminal surface",
            )?,
            "consoleSize" => absent(&path, field, "a terminal (see process.terminal)")?,
            "user" => user(field)?,
            "commandLine" => absent(&path, field, "a Windows host")?,
            "capabilities" => {
                for (set, capabilities) in field.as_object().into_iter().flatten() {
                    let mut names: Vec<&str> = capabilities
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                        .collect();
                    names.sort_unstable();
                    let mut ours: Vec<&str> = match set.as_str() {
                        "bounding" | "effective" | "permitted" => CAPABILITIES.to_vec(),
                        "inheritable" | "ambient" => Vec::new(),
                        _ => {
                            return Err(refused(
                                format!("{path}.{set}"),
                                "a field this runtime does not know",
                            ));
                        }
                    };
                    ours.sort_unstable();
                    if names != ours {
                        return Err(refused(
                            format!("{path}.{set}"),
                            format!(
                                "{capabilities}: the kernel gives {ours:?}; others need the kernel \
                                 to take capabilities at start"
                            ),
                        ));
                    }
                }
            }
            "rlimits" => {
                for (index, limit) in field.as_array().into_iter().flatten().enumerate() {
                    let kind = limit["type"].as_str().unwrap_or("");
                    let (soft, hard) = LIMITS
                        .iter()
                        .find(|(name, ..)| *name == kind)
                        .map_or((u64::MAX, u64::MAX), |&(_, soft, hard)| (soft, hard));
                    if limit["soft"].as_u64() != Some(soft) || limit["hard"].as_u64() != Some(hard)
                    {
                        return Err(refused(
                            format!("{path}[{index}]"),
                            format!(
                                "{limit}: the kernel starts {kind} at soft {soft}, hard {hard}; \
                                 others need the kernel to take limits at start"
                            ),
                        ));
                    }
                }
            }
            "noNewPrivileges" => absent(
                &path,
                field,
                "the kernel setting no_new_privs on the first process at start",
            )?,
            "apparmorProfile" | "selinuxLabel" => {
                absent(&path, field, "a security module in the kernel")?;
            }
            "oomScoreAdj" | "ioPriority" | "scheduler" | "execCPUAffinity" => {
                absent(&path, field, "the kernel taking the value at start")?;
            }
            _ => return Err(refused(path, "a field this runtime does not know")),
        }
    }
    Ok(())
}

fn user(value: &Value) -> Result<(), Error> {
    for (name, field) in value.as_object().into_iter().flatten() {
        let path = format!("process.user.{name}");
        match name.as_str() {
            "additionalGids" => {}
            "uid" | "gid" => {
                if field.as_u64() != Some(0) {
                    return Err(refused(
                        path,
                        format!(
                            "{field}: the kernel starts the first process as root; others need it to take the user at start"
                        ),
                    ));
                }
            }
            "umask" => {
                if field.as_u64() != Some(UMASK) {
                    return Err(refused(
                        path,
                        format!(
                            "{field}: the kernel starts with umask 0022; others need it to take the umask at start"
                        ),
                    ));
                }
            }
            "username" => absent(&path, field, "a Windows host")?,
            _ => return Err(refused(path, "a field this runtime does not know")),
        }
    }
    Ok(())
}

fn linux(value: &Value) -> Result<(), Error> {
    for (name, field) in value.as_object().into_iter().flatten() {
        let path = format!("linux.{name}");
        match name.as_str() {
            "namespaces" => namespaces(field)?,
            "uidMappings" | "gidMappings" => absent(&path, field, "user namespaces in the kernel")?,
            "sysctl" => absent(&path, field, "the kernel taking its settings at start")?,
            "resources" | "cgroupsPath" => absent(
                &path,
                field,
                "cgroups: the kernel accounting CPU, memory, pids and I/O per group and enforcing \
                 limits",
            )?,
            "devices" => absent(
                &path,
                field,
                "device nodes in the kernel for each, with the outside behind them",
            )?,
            "seccomp" => absent(
                &path,
                field,
                "the kernel installing the filter on the first process at start",
            )?,
            "rootfsPropagation" | "mountLabel" | "maskedPaths" | "readonlyPaths" => absent(
                &path,
                field,
                "read-only, masking and labelled mounts in the kernel",
            )?,
            "intelRdt" | "memoryPolicy" | "personality" | "timeOffsets" | "netDevices" => {
                absent(&path, field, "the kernel taking the value at start")?;
            }
            _ => return Err(refused(path, "a field this runtime does not know")),
        }
    }
    Ok(())
}

/// Each container has its own namespace of every kind the kernel has, so the list must name exactly
/// those, none joined by path.
fn namespaces(value: &Value) -> Result<(), Error> {
    let given = value.as_array().map(Vec::as_slice).unwrap_or_default();
    for (index, namespace) in given.iter().enumerate() {
        let field = format!("linux.namespaces[{index}]");
        let kind = namespace["type"].as_str().unwrap_or("");
        if !empty(&namespace["path"]) {
            return Err(refused(
                field,
                format!(
                    "{namespace}: joining another's namespace needs several containers in one kernel"
                ),
            ));
        }
        if !NAMESPACES.contains(&kind) {
            return Err(refused(
                field,
                format!("{namespace}: {kind} namespaces in the kernel"),
            ));
        }
    }
    for kind in NAMESPACES {
        if !given.iter().any(|namespace| namespace["type"] == kind) {
            return Err(refused(
                "linux.namespaces",
                format!("lacks {kind}: the container would share the host's, which it never sees"),
            ));
        }
    }
    Ok(())
}
