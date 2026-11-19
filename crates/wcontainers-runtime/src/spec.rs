//! Every field of the OCI runtime configuration honoured or refused by name. A property of a
//! configuration's JSON that the specification does not define is ignored and named, for a warning.

use oci_spec::runtime::Spec;
use serde_json::{Value, json};

use crate::{Error, TerminalSize};

/// The powers the kernel gives the first process: a default Docker container's.
const CAPABILITIES: &[&str] = &[
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
const LIMITS: &[(&str, u64, u64)] = &[
    ("RLIMIT_STACK", 8 << 20, u64::MAX),
    ("RLIMIT_CORE", 0, u64::MAX),
    ("RLIMIT_NOFILE", 1 << 20, 1 << 20),
    ("RLIMIT_NICE", 0, 0),
    ("RLIMIT_RTPRIO", 0, 0),
    ("RLIMIT_SIGPENDING", 128_302, 128_302),
    ("RLIMIT_MSGQUEUE", 819_200, 819_200),
];

/// The namespaces every container has its own of, by construction.
const NAMESPACES: &[&str] = &["pid", "network", "ipc", "uts", "mount", "cgroup"];

/// The umask the kernel starts the first process with.
const UMASK: u64 = 0o022;

/// The paths the kernel mounts again read-only, as a default Docker container has the first of its.
const READONLY_PATHS: &[&str] = &["/proc/sys"];

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

/// The configuration's field that lists its mounts.
const MOUNTS: &str = "mounts";

/// A mount's field path, as the refusals name it.
pub(crate) fn mount_field(index: usize) -> String {
    format!("{MOUNTS}[{index}]")
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
        "linux": {"namespaces": namespaces, "readonlyPaths": READONLY_PATHS},
    }))
    .expect("the base configuration is one oci-spec reads")
}

/// A configuration from its JSON, and the path of each property in it that the specification, as
/// oci-spec's types read it, does not define, and of each annotation (this runtime acts on none).
/// Those are ignored, as the specification's extensibility rule says ("Runtimes MAY log unknown
/// properties but MUST otherwise ignore them"), for the caller to warn of; a defined field this
/// runtime cannot honour is refused when the container is made.
pub fn spec_from_json(json: &[u8]) -> Result<(Spec, Vec<String>), Error> {
    let failed = |error: serde_json::Error| Error::Configuration(error.to_string());
    let mut ignored = Vec::new();
    let mut deserializer = serde_json::Deserializer::from_slice(json);
    let spec: Spec = serde_ignored::deserialize(&mut deserializer, |path| {
        ignored.push(path_name(&path));
    })
    .map_err(failed)?;
    deserializer.end().map_err(failed)?;
    if let Some(annotations) = spec.annotations() {
        ignored.extend(annotations.keys().map(|key| format!("annotations.{key}")));
    }
    Ok((spec, ignored))
}

/// A path as the refusals name fields: `process.args`, `mounts[2].options`.
fn path_name(path: &serde_ignored::Path) -> String {
    use serde_ignored::Path;
    match path {
        Path::Root => String::new(),
        Path::Seq { parent, index } => format!("{}[{index}]", path_name(parent)),
        Path::Map { parent, key } => match path_name(parent) {
            parent if parent.is_empty() => key.clone(),
            parent => format!("{parent}.{key}"),
        },
        Path::Some { parent }
        | Path::NewtypeStruct { parent }
        | Path::NewtypeVariant { parent } => path_name(parent),
    }
}

/// What the kernel's start takes from an accepted configuration.
pub(crate) struct Accepted {
    pub args: Vec<String>,
    pub env: Vec<String>,
    pub cwd: String,
    pub hostname: String,
    pub groups: Vec<u32>,
    pub uid: u32,
    /// The terminal the first process has, when it has one.
    pub terminal: Option<TerminalSize>,
    /// The bind mount of a host directory, if one is given.
    pub volume: Option<Volume>,
}

/// A directory of the host bind-mounted into the container, served by the system's file backend.
pub(crate) struct Volume {
    /// Its place among the configuration's mounts, to name it in a refusal.
    pub index: usize,
    pub source: std::path::PathBuf,
    pub destination: String,
}

/// The mount type of a bind mount, and the options that make one.
const BIND: &str = "bind";
const RBIND: &str = "rbind";
const BIND_OPTIONS: &[&str] = &[BIND, RBIND];

/// The other options of a bind mount that the kernel's mount of the file backend has: read and
/// write, and no propagation to or from the host.
const READ_WRITE: &str = "rw";
const HONOURED_OPTIONS: &[&str] = &[READ_WRITE, "private", "rprivate"];

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
            MOUNTS => {}
            "process" => process(field)?,
            "hostname" | "annotations" => {}
            "domainname" => absent(name, field, "the kernel taking a domain name at start")?,
            "hooks" => absent(
                name,
                field,
                "the local system running host programs at the points the runtime \
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
    let volume = check_mounts(fields.get(MOUNTS).unwrap_or(&Value::Null))?;
    if !fields.contains_key("linux") {
        namespaces(&Value::Null)?;
    }
    let linux = fields.get("linux").and_then(Value::as_object);
    if !linux.is_some_and(|linux| linux.contains_key("readonlyPaths")) {
        readonly_paths(&Value::Null)?;
    }
    let process = spec
        .process()
        .as_ref()
        .ok_or_else(|| refused("process", "a container needs a process to run"))?;
    if process.capabilities().is_none() {
        return Err(refused(
            "process.capabilities",
            format!(
                "absent, so the process would keep no capabilities; the kernel gives {CAPABILITIES:?}, \
                 others need it to take capabilities at start"
            ),
        ));
    }
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
        // As runc, an empty name sets none.
        hostname: spec
            .hostname()
            .clone()
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| "localhost".to_string()),
        groups: process.user().additional_gids().clone().unwrap_or_default(),
        uid: process.user().uid(),
        terminal: terminal(process)?,
        volume,
    })
}

fn root(value: &Value) -> Result<(), Error> {
    for (name, field) in value.as_object().into_iter().flatten() {
        match name.as_str() {
            "path" => absent(
                "root.path",
                field,
                "the root's files are the image argument; the local system's file backend serves a \
                 host directory with the host user's rights, so a root from one would show every file \
                 owned by that user and could make no set-id or device files, unlike the image's own \
                 root; it needs an image read from a directory and owners mapped (user namespaces)",
            )?,
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

/// The mounts must be the kernel's own, each as it makes it and in its order: it makes all of them,
/// once each, and the standard makes them in the listed order. After them may come one bind mount of
/// a host directory, which the kernel mounts last, as its file backend.
fn check_mounts(value: &Value) -> Result<Option<Volume>, Error> {
    let kernel = mounts();
    let kernel = kernel.as_array().expect("a list");
    let mut given = value.as_array().map(Vec::as_slice).unwrap_or_default();
    let mut volume = None;
    if given.len() > kernel.len() {
        let index = kernel.len();
        volume = Some(check_bind_mount(index, &given[index])?);
        if given.len() > index + 1 {
            return Err(refused(
                mount_field(index + 1),
                "one directory of the host per container: the kernel takes one file backend",
            ));
        }
        given = &given[..index];
    }
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
    let same = |ours: &Value, mount: &Value| {
        ours["destination"] == mount["destination"]
            && ours["type"] == mount["type"]
            && ours["source"] == mount["source"]
            && options(ours) == options(mount)
    };
    for (index, mount) in given.iter().enumerate() {
        let field = mount_field(index);
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
        if !kernel.iter().any(|ours| same(ours, mount)) {
            return Err(refused(
                field,
                format!(
                    "{mount}: only the kernel's own mounts are made; bind mounts of host \
                     directories need the local system's file backend, other file systems the \
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
                MOUNTS,
                format!(
                    "lacks {}, which the kernel always mounts; leaving it out needs the kernel to \
                     take its mounts at start",
                    ours["destination"]
                ),
            ));
        }
    }
    if let Some(index) = (0..given.len()).find(|&index| {
        kernel
            .get(index)
            .is_none_or(|ours| !same(ours, &given[index]))
    }) {
        let order: Vec<&str> = kernel
            .iter()
            .filter_map(|ours| ours["destination"].as_str())
            .collect();
        return Err(refused(
            mount_field(index),
            format!(
                "{}: the kernel makes its mounts once each, in the order {}; another order or a \
                 repeat needs the kernel to take its mounts at start",
                given[index],
                order.join(", ")
            ),
        ));
    }
    Ok(volume)
}

/// The bind mount of host directory `source` at `destination`, read and write, as this runtime
/// honours one: to be added after the configuration's other mounts.
pub fn bind_mount(source: &std::path::Path, destination: &str) -> oci_spec::runtime::Mount {
    let mut mount = oci_spec::runtime::Mount::default();
    mount.set_destination(destination.into());
    mount.set_typ(Some(BIND.to_string()));
    mount.set_source(Some(source.to_path_buf()));
    mount.set_options(Some(vec![RBIND.to_string(), READ_WRITE.to_string()]));
    mount
}

/// The terminal `process.terminal` asks for, of `process.consoleSize`'s size (0 by 0 without one, as
/// `docker run --tty` gives when its client has no terminal); the size is ignored without a
/// terminal, as the specification says.
fn terminal(process: &oci_spec::runtime::Process) -> Result<Option<TerminalSize>, Error> {
    if process.terminal() != Some(true) {
        return Ok(None);
    }
    let Some(size) = process.console_size() else {
        return Ok(Some(TerminalSize::default()));
    };
    let fit = |value: u64, name: &str| {
        u16::try_from(value).map_err(|_| {
            refused(
                format!("process.consoleSize.{name}"),
                format!(
                    "{value}: a terminal's size is at most {} characters",
                    u16::MAX
                ),
            )
        })
    };
    Ok(Some(TerminalSize {
        columns: fit(size.width(), "width")?,
        rows: fit(size.height(), "height")?,
    }))
}

/// A bind mount of a host directory, read and write, at an absolute path other than the root.
fn check_bind_mount(index: usize, mount: &Value) -> Result<Volume, Error> {
    let field = mount_field(index);
    let options: Vec<&str> = mount["options"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    let bind = mount["type"].as_str() == Some(BIND)
        || options.iter().any(|option| BIND_OPTIONS.contains(option));
    if !bind {
        return Err(refused(
            &field,
            format!(
                "{mount}: of the kernel's own mounts and bind mounts of host directories only these                  are made; other file systems need the kernel"
            ),
        ));
    }
    if let Some(option) = options
        .iter()
        .find(|option| !BIND_OPTIONS.contains(option) && !HONOURED_OPTIONS.contains(option))
    {
        return Err(refused(
            format!("{field}.options"),
            format!(
                "{option}: a host directory is mounted read and write with no propagation; other                  options need the kernel's mounts to take them"
            ),
        ));
    }
    let destination = mount["destination"].as_str().unwrap_or_default();
    if !destination.starts_with('/') || destination.trim_end_matches('/').is_empty() {
        return Err(refused(
            format!("{field}.destination"),
            format!("{destination:?}: a bind mount needs an absolute path other than the root"),
        ));
    }
    let source = mount["source"].as_str().unwrap_or_default();
    if !source.starts_with('/') {
        return Err(refused(
            format!("{field}.source"),
            format!("{source:?}: a host directory is named by its absolute path"),
        ));
    }
    Ok(Volume {
        index,
        source: source.into(),
        destination: destination.to_string(),
    })
}

fn process(value: &Value) -> Result<(), Error> {
    for (name, field) in value.as_object().into_iter().flatten() {
        let path = format!("process.{name}");
        match name.as_str() {
            "args" | "env" | "cwd" => {}
            "terminal" | "consoleSize" => {}
            "user" => user(field)?,
            "commandLine" => absent(&path, field, "a Windows host")?,
            "capabilities" => {
                let sets = field.as_object().cloned().unwrap_or_default();
                if let Some(set) = ["bounding", "effective", "permitted"]
                    .into_iter()
                    .find(|set| !sets.contains_key(*set))
                {
                    return Err(refused(
                        format!("{path}.{set}"),
                        format!(
                            "absent, so the set is empty; the kernel gives {CAPABILITIES:?}, others \
                             need it to take capabilities at start"
                        ),
                    ));
                }
                for (set, capabilities) in &sets {
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
                "device nodes in the kernel for each, with the system behind them",
            )?,
            "seccomp" => absent(
                &path,
                field,
                "the kernel installing the filter on the first process at start",
            )?,
            "readonlyPaths" => readonly_paths(field)?,
            "rootfsPropagation" | "mountLabel" | "maskedPaths" => absent(
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

/// The kernel mounts its read-only paths whatever the configuration says, so the list must name
/// exactly those.
fn readonly_paths(value: &Value) -> Result<(), Error> {
    let mut given: Vec<&str> = value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    given.sort_unstable();
    if given != READONLY_PATHS {
        return Err(refused(
            "linux.readonlyPaths",
            format!(
                "{}: the kernel makes {READONLY_PATHS:?} read-only; others need read-only mounts \
                 in the kernel",
                if value.is_null() {
                    "absent".to_string()
                } else {
                    value.to_string()
                }
            ),
        ));
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
    for &kind in NAMESPACES {
        if !given.iter().any(|namespace| namespace["type"] == kind) {
            return Err(refused(
                "linux.namespaces",
                format!("lacks {kind}: the container would share the host's, which it never sees"),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A configuration as `runc spec` writes it, with resources, seccomp and devices added: every
    /// field is one oci-spec reads, so none is ignored; a property deep in it that the
    /// specification does not define is ignored and named by its path.
    #[test]
    fn a_whole_configuration_is_read_with_nothing_unread() {
        let given = json!({
            "ociVersion": "1.0.2-dev",
            "process": {
                "terminal": true,
                "user": {"uid": 0, "gid": 0},
                "args": ["sh"],
                "env": ["PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin", "TERM=xterm"],
                "cwd": "/",
                "capabilities": {
                    "bounding": ["CAP_AUDIT_WRITE", "CAP_KILL", "CAP_NET_BIND_SERVICE"],
                    "effective": ["CAP_AUDIT_WRITE", "CAP_KILL", "CAP_NET_BIND_SERVICE"],
                    "permitted": ["CAP_AUDIT_WRITE", "CAP_KILL", "CAP_NET_BIND_SERVICE"],
                    "ambient": ["CAP_AUDIT_WRITE", "CAP_KILL", "CAP_NET_BIND_SERVICE"]
                },
                "rlimits": [{"type": "RLIMIT_NOFILE", "hard": 1024, "soft": 1024}],
                "noNewPrivileges": true
            },
            "root": {"path": "rootfs", "readonly": true},
            "hostname": "runc",
            "mounts": [
                {"destination": "/proc", "type": "proc", "source": "proc"},
                {"destination": "/dev", "type": "tmpfs", "source": "tmpfs",
                 "options": ["nosuid", "strictatime", "mode=755", "size=65536k"]}
            ],
            "linux": {
                "resources": {
                    "devices": [{"allow": false, "access": "rwm"}],
                    "memory": {"limit": 1048576},
                    "pids": {"limit": 10}
                },
                "namespaces": [{"type": "pid"}, {"type": "network"}, {"type": "ipc"},
                               {"type": "uts"}, {"type": "mount"}, {"type": "cgroup"}],
                "seccomp": {
                    "defaultAction": "SCMP_ACT_ERRNO",
                    "architectures": ["SCMP_ARCH_AARCH64"],
                    "syscalls": [{"names": ["getcwd"], "action": "SCMP_ACT_ALLOW"}]
                },
                "maskedPaths": ["/proc/acpi", "/proc/kcore"],
                "readonlyPaths": ["/proc/bus", "/proc/sys"]
            }
        });
        let (_, ignored) = spec_from_json(&serde_json::to_vec(&given).unwrap()).unwrap();
        assert_eq!(ignored, Vec::<String>::new());
        let mut unknown = given;
        unknown["linux"]["resources"]["memory"]["limitt"] = json!(1);
        let (_, ignored) = spec_from_json(&serde_json::to_vec(&unknown).unwrap()).unwrap();
        assert_eq!(ignored, ["linux.resources.memory.limitt"]);
    }
}
