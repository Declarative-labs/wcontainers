//! Every field of the OCI runtime configuration honoured or refused by name.
//!
//! A property of a configuration's JSON that the specification does not define is ignored and named, for a warning.

use oci_spec::runtime::Spec;
use serde_json::{Value, json};

use crate::image::{ROOT_ID, ROOT_PATH};
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

/// The limits the kernel starts the first process with, as (name, soft, hard); every other limit is unlimited.
///
/// https://github.com/stack1ng/wcontainers/blob/main/docs/runtime.md#18-measurements-behind-the-decisions
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

/// The umask the kernel starts the first process with, the usual default.
///
/// https://man7.org/linux/man-pages/man2/umask.2.html
const UMASK: u64 = 0o022;

/// The version of the runtime specification the base configuration is written to, and the major version read.
const OCI_VERSION: &str = "1.2.1";
const READ_VERSION_START: &str = "1.";

/// The first process's search path in the base configuration, as Docker's.
pub(crate) const DEFAULT_PATH: &str =
    "PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin";

/// The host name of a container whose configuration names none.
const DEFAULT_HOSTNAME: &str = "localhost";

/// What starts an absolute path.
const ABSOLUTE_START: char = '/';

/// What joins a field path's parts, and what holds an element's index.
const MEMBER_SEPARATOR: char = '.';
const INDEX_OPEN: char = '[';
const INDEX_CLOSE: char = ']';
/// What joins the destinations of the kernel's mounts in a refusal.
const LIST_SEPARATOR: &str = ", ";

// The paths the kernel mounts again read-only, and those it masks, as a default Docker container has those of its
// lists that its file systems have.
use wcontainers_kernel_linux::{
    DOCKER_MASKED_PATHS as MASKED_PATHS, DOCKER_READ_ONLY_PATHS as READONLY_PATHS,
};

/// The fields of the configuration's `linux` that list the kernel's read-only and masked paths.
pub const READONLY_PATHS_FIELD: &str = "readonlyPaths";
pub const MASKED_PATHS_FIELD: &str = "maskedPaths";

/// The keys of the configuration's JSON that this runtime reads, by their names in the runtime specification.
const OCI_VERSION_KEY: &str = "ociVersion";
const ROOT_KEY: &str = "root";
const MOUNTS_KEY: &str = "mounts";
const PROCESS_KEY: &str = "process";
const HOSTNAME_KEY: &str = "hostname";
const ANNOTATIONS_KEY: &str = "annotations";
const DOMAINNAME_KEY: &str = "domainname";
const HOOKS_KEY: &str = "hooks";
const LINUX_KEY: &str = "linux";
const UID_MAPPINGS_KEY: &str = "uidMappings";
const GID_MAPPINGS_KEY: &str = "gidMappings";
const SOLARIS_KEY: &str = "solaris";
const WINDOWS_KEY: &str = "windows";
const VM_KEY: &str = "vm";
const ZOS_KEY: &str = "zos";
const PATH_KEY: &str = "path";
const READONLY_KEY: &str = "readonly";
const DESTINATION_KEY: &str = "destination";
const TYPE_KEY: &str = "type";
const SOURCE_KEY: &str = "source";
const OPTIONS_KEY: &str = "options";
const ARGS_KEY: &str = "args";
const ENV_KEY: &str = "env";
const CWD_KEY: &str = "cwd";
const TERMINAL_KEY: &str = "terminal";
const CONSOLE_SIZE_KEY: &str = "consoleSize";
const WIDTH_KEY: &str = "width";
const HEIGHT_KEY: &str = "height";
const USER_KEY: &str = "user";
const COMMAND_LINE_KEY: &str = "commandLine";
const CAPABILITIES_KEY: &str = "capabilities";
const BOUNDING_KEY: &str = "bounding";
const EFFECTIVE_KEY: &str = "effective";
const PERMITTED_KEY: &str = "permitted";
const INHERITABLE_KEY: &str = "inheritable";
const AMBIENT_KEY: &str = "ambient";
const RLIMITS_KEY: &str = "rlimits";
const SOFT_KEY: &str = "soft";
const HARD_KEY: &str = "hard";
const NO_NEW_PRIVILEGES_KEY: &str = "noNewPrivileges";
const APPARMOR_PROFILE_KEY: &str = "apparmorProfile";
const SELINUX_LABEL_KEY: &str = "selinuxLabel";
const OOM_SCORE_ADJ_KEY: &str = "oomScoreAdj";
const IO_PRIORITY_KEY: &str = "ioPriority";
const SCHEDULER_KEY: &str = "scheduler";
const EXEC_CPU_AFFINITY_KEY: &str = "execCPUAffinity";
const ADDITIONAL_GIDS_KEY: &str = "additionalGids";
const UID_KEY: &str = "uid";
const GID_KEY: &str = "gid";
const UMASK_KEY: &str = "umask";
const USERNAME_KEY: &str = "username";
const NAMESPACES_KEY: &str = "namespaces";
const SYSCTL_KEY: &str = "sysctl";
const RESOURCES_KEY: &str = "resources";
const CGROUPS_PATH_KEY: &str = "cgroupsPath";
const DEVICES_KEY: &str = "devices";
const SECCOMP_KEY: &str = "seccomp";
const ROOTFS_PROPAGATION_KEY: &str = "rootfsPropagation";
const MOUNT_LABEL_KEY: &str = "mountLabel";
const INTEL_RDT_KEY: &str = "intelRdt";
const MEMORY_POLICY_KEY: &str = "memoryPolicy";
const PERSONALITY_KEY: &str = "personality";
const TIME_OFFSETS_KEY: &str = "timeOffsets";
const NET_DEVICES_KEY: &str = "netDevices";

/// The capability sets the kernel fills, and those it leaves empty.
const FILLED_SETS: &[&str] = &[BOUNDING_KEY, EFFECTIVE_KEY, PERMITTED_KEY];
const EMPTY_SETS: &[&str] = &[INHERITABLE_KEY, AMBIENT_KEY];

/// The keys a mount of the kernel's has, and so the only ones a mount may have without user namespaces.
const MOUNT_KEYS: &[&str] = &[DESTINATION_KEY, TYPE_KEY, SOURCE_KEY, OPTIONS_KEY];

/// One mount the kernel always makes, as a default Docker container has it.
struct KernelMount {
    destination: &'static str,
    kind: &'static str,
    source: &'static str,
    options: &'static [&'static str],
}

/// The mounts the kernel always makes, in its order.
const KERNEL_MOUNTS: &[KernelMount] = &[
    KernelMount {
        destination: "/proc",
        kind: "proc",
        source: "proc",
        options: &["nosuid", "noexec", "nodev"],
    },
    KernelMount {
        destination: "/dev",
        kind: "tmpfs",
        source: "tmpfs",
        options: &["nosuid", "strictatime", "mode=755", "size=65536k"],
    },
    KernelMount {
        destination: "/dev/pts",
        kind: "devpts",
        source: "devpts",
        options: &[
            "nosuid",
            "noexec",
            "newinstance",
            "ptmxmode=0666",
            "mode=0620",
            "gid=5",
        ],
    },
    KernelMount {
        destination: "/sys",
        kind: "sysfs",
        source: "sysfs",
        options: &["nosuid", "noexec", "nodev", "ro"],
    },
    KernelMount {
        destination: "/dev/mqueue",
        kind: "mqueue",
        source: "mqueue",
        options: &["nosuid", "noexec", "nodev"],
    },
    KernelMount {
        destination: "/dev/shm",
        kind: "tmpfs",
        source: "shm",
        options: &["nosuid", "noexec", "nodev", "mode=1777", "size=65536k"],
    },
];

/// Returns the mounts the kernel always makes as the configuration's JSON lists them.
fn mounts() -> Value {
    KERNEL_MOUNTS
        .iter()
        .map(|mount| {
            json!({
                DESTINATION_KEY: mount.destination,
                TYPE_KEY: mount.kind,
                SOURCE_KEY: mount.source,
                OPTIONS_KEY: mount.options,
            })
        })
        .collect()
}

/// Returns the path of the member `name` of the field at `parent`, as the refusals name fields.
fn member(parent: &str, name: &str) -> String {
    format!("{parent}{MEMBER_SEPARATOR}{name}")
}

/// Returns the path of the element `index` of the list at `parent`.
fn element(parent: &str, index: usize) -> String {
    format!("{parent}{INDEX_OPEN}{index}{INDEX_CLOSE}")
}

/// Returns a mount's field path, as the refusals name it.
pub(crate) fn mount_field(index: usize) -> String {
    element(MOUNTS_KEY, index)
}

/// Returns the configuration of everything the kernel gives a container, with no program.
///
/// It is what an image's configuration is converted into, and where a configuration for a plain tar starts.
pub fn base_spec() -> Spec {
    let capabilities = json!(CAPABILITIES);
    let namespaces: Vec<Value> = NAMESPACES
        .iter()
        .map(|kind| json!({ TYPE_KEY: kind }))
        .collect();
    serde_json::from_value(json!({
        OCI_VERSION_KEY: OCI_VERSION,
        PROCESS_KEY: {
            USER_KEY: { UID_KEY: ROOT_ID, GID_KEY: ROOT_ID },
            CWD_KEY: ROOT_PATH,
            ENV_KEY: [DEFAULT_PATH],
            CAPABILITIES_KEY: {
                BOUNDING_KEY: capabilities,
                EFFECTIVE_KEY: capabilities,
                PERMITTED_KEY: capabilities,
            },
        },
        MOUNTS_KEY: mounts(),
        LINUX_KEY: {
            NAMESPACES_KEY: namespaces,
            READONLY_PATHS_FIELD: READONLY_PATHS,
            MASKED_PATHS_FIELD: MASKED_PATHS,
        },
    }))
    .expect("the base configuration is one oci-spec reads")
}

/// Returns a configuration from its JSON, and the path of each property in it that the specification does not define.
///
/// Properties oci-spec does not read and annotations, which this runtime acts on none of, are ignored as the
/// specification's extensibility rule says, for the caller to warn of; a field it cannot honour is refused later.
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
        ignored.extend(annotations.keys().map(|key| member(ANNOTATIONS_KEY, key)));
    }
    Ok((spec, ignored))
}

/// Returns a path as the refusals name fields: `process.args`, `mounts[2].options`.
fn path_name(path: &serde_ignored::Path) -> String {
    use serde_ignored::Path;
    match path {
        Path::Root => String::new(),
        Path::Seq { parent, index } => element(&path_name(parent), *index),
        Path::Map { parent, key } => match path_name(parent) {
            parent if parent.is_empty() => key.clone(),
            parent => member(&parent, key),
        },
        Path::Some { parent }
        | Path::NewtypeStruct { parent }
        | Path::NewtypeVariant { parent } => path_name(parent),
    }
}

/// What the kernel's start takes from an accepted configuration.
pub(crate) struct Accepted {
    pub(crate) args: Vec<String>,
    pub(crate) env: Vec<String>,
    pub(crate) cwd: String,
    pub(crate) hostname: String,
    pub(crate) groups: Vec<u32>,
    pub(crate) uid: u32,
    /// The terminal the first process has, when it has one.
    pub(crate) terminal: Option<TerminalSize>,
    /// The bind mount of a host directory, if one is given.
    pub(crate) volume: Option<Volume>,
}

/// A directory of the host bind-mounted into the container, served by the system's file backend.
pub(crate) struct Volume {
    /// Its place among the configuration's mounts, to name it in a refusal.
    pub(crate) index: usize,
    pub(crate) source: std::path::PathBuf,
    pub(crate) destination: String,
}

/// The mount type of a bind mount, and the options that make one.
const BIND: &str = "bind";
const RBIND: &str = "rbind";
const BIND_OPTIONS: &[&str] = &[BIND, RBIND];

/// The other options of a bind mount that the kernel's mount of the file backend has: read and write, no propagation.
const READ_WRITE: &str = "rw";
const HONOURED_OPTIONS: &[&str] = &[READ_WRITE, "private", "rprivate"];

/// Refusal reasons given for more than one field.
const UNKNOWN_FIELD: &str = "a field this runtime does not know";
const NEEDS_USER_NAMESPACES: &str = "user namespaces in the kernel";
const NEEDS_WINDOWS: &str = "a Windows host";
const NEEDS_VALUE_AT_START: &str = "the kernel taking the value at start";

/// The refusal of a list of the kernel's read-only or masked paths that names others: what the kernel makes of its own.
const MAKES_READ_ONLY: &str = "read-only; others need read-only mounts in the kernel";
const MAKES_MASKED: &str = "masked; others need masking mounts in the kernel";
/// The refusal of a propagation or a label of mounts, which need the kernel to propagate and label them.
const NEEDS_LABELLED_MOUNTS: &str = "propagation and labelled mounts in the kernel";

/// Refusal reasons given for one field each.
const NOT_AN_OBJECT: &str = "the configuration is not a JSON object";
const NEEDS_DOMAIN_NAME: &str = "the kernel taking a domain name at start";
const NEEDS_HOOKS: &str =
    "the local system running host programs at the points the runtime specification names";
const NEEDS_LINUX: &str = "another kind of host than Linux";
const NEEDS_PROCESS: &str = "a container needs a process to run";
const NEEDS_PROGRAM: &str = "a container needs a program to run";
const NEEDS_ROOT_FROM_DIRECTORY: &str = "the root's files are the image argument; the local system's file backend serves a \
     host directory with the host user's rights, so a root from one would show every file owned by that user and \
     could make no set-id or device files, unlike the image's own root; it needs an image read from a directory and \
     owners mapped (user namespaces)";
const NEEDS_READ_ONLY_MOUNTS: &str = "read-only mounts in the kernel";
const ONE_DIRECTORY: &str =
    "one directory of the host per container: the kernel takes one file backend";
const NEEDS_NO_NEW_PRIVILEGES: &str =
    "the kernel setting no_new_privs on the first process at start";
const NEEDS_SECURITY_MODULE: &str = "a security module in the kernel";
const NEEDS_SETTINGS: &str = "the kernel taking its settings at start";
const NEEDS_CGROUPS: &str =
    "cgroups: the kernel accounting CPU, memory, pids and I/O per group and enforcing limits";
const NEEDS_DEVICES: &str = "device nodes in the kernel for each, with the system behind them";
const NEEDS_SECCOMP: &str = "the kernel installing the filter on the first process at start";

fn refused(field: impl Into<String>, reason: impl Into<String>) -> Error {
    Error::Refused {
        field: field.into(),
        reason: reason.into(),
    }
}

/// Returns whether a value says nothing: absent, null, false, or an empty list, map or string.
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
fn refuse_unless_absent(field: &str, value: &Value, takes: &str) -> Result<(), Error> {
    if empty(value) {
        Ok(())
    } else {
        Err(refused(field, format!("{value}: {takes}")))
    }
}

/// Checks every field of `spec`, and returns what the kernel's start takes from it.
pub(crate) fn accept(spec: &Spec) -> Result<Accepted, Error> {
    let value = serde_json::to_value(spec).expect("a configuration is JSON");
    let Value::Object(fields) = &value else {
        return Err(refused(String::new(), NOT_AN_OBJECT));
    };
    for (name, field) in fields {
        match name.as_str() {
            OCI_VERSION_KEY => {
                if !field
                    .as_str()
                    .is_some_and(|version| version.starts_with(READ_VERSION_START))
                {
                    return Err(refused(
                        name,
                        format!("{field}: this runtime reads version 1"),
                    ));
                }
            }
            ROOT_KEY => check_root(field)?,
            MOUNTS_KEY => {}
            PROCESS_KEY => process(field)?,
            HOSTNAME_KEY | ANNOTATIONS_KEY => {}
            DOMAINNAME_KEY => refuse_unless_absent(name, field, NEEDS_DOMAIN_NAME)?,
            HOOKS_KEY => refuse_unless_absent(name, field, NEEDS_HOOKS)?,
            LINUX_KEY => check_linux(field)?,
            UID_MAPPINGS_KEY | GID_MAPPINGS_KEY => {
                refuse_unless_absent(name, field, NEEDS_USER_NAMESPACES)?
            }
            SOLARIS_KEY | WINDOWS_KEY | VM_KEY | ZOS_KEY => {
                refuse_unless_absent(name, field, NEEDS_LINUX)?;
            }
            _ => return Err(refused(name, UNKNOWN_FIELD)),
        }
    }
    // The kernel makes its mounts and namespaces whatever the configuration says, so leaving the lists out is refused
    // as leaving one of them out is.
    let volume = check_mounts(fields.get(MOUNTS_KEY).unwrap_or(&Value::Null))?;
    if !fields.contains_key(LINUX_KEY) {
        check_namespaces(&Value::Null)?;
    }
    let linux = fields.get(LINUX_KEY).and_then(Value::as_object);
    // The kernel's read-only and masked paths likewise, each list left out checked as one given empty would be.
    for field in [READONLY_PATHS_FIELD, MASKED_PATHS_FIELD] {
        if !linux.is_some_and(|linux| linux.contains_key(field)) {
            check_linux(&json!({ field: Value::Null }))?;
        }
    }
    let process = spec
        .process()
        .as_ref()
        .ok_or_else(|| refused(PROCESS_KEY, NEEDS_PROCESS))?;
    if process.capabilities().is_none() {
        return Err(refused(
            member(PROCESS_KEY, CAPABILITIES_KEY),
            format!(
                "absent, so the process would keep no capabilities; the kernel gives {CAPABILITIES:?}, \
                 others need it to take capabilities at start"
            ),
        ));
    }
    let args = process.args().clone().unwrap_or_default();
    if args.is_empty() {
        return Err(refused(member(PROCESS_KEY, ARGS_KEY), NEEDS_PROGRAM));
    }
    let cwd = process.cwd().to_string_lossy().into_owned();
    if !cwd.starts_with(ABSOLUTE_START) {
        return Err(refused(
            member(PROCESS_KEY, CWD_KEY),
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
            .unwrap_or_else(|| DEFAULT_HOSTNAME.to_string()),
        groups: process.user().additional_gids().clone().unwrap_or_default(),
        uid: process.user().uid(),
        terminal: terminal(process)?,
        volume,
    })
}

fn check_root(value: &Value) -> Result<(), Error> {
    for (name, field) in value.as_object().into_iter().flatten() {
        match name.as_str() {
            PATH_KEY => refuse_unless_absent(
                &member(ROOT_KEY, PATH_KEY),
                field,
                NEEDS_ROOT_FROM_DIRECTORY,
            )?,
            READONLY_KEY => refuse_unless_absent(
                &member(ROOT_KEY, READONLY_KEY),
                field,
                NEEDS_READ_ONLY_MOUNTS,
            )?,
            _ => {
                return Err(refused(member(ROOT_KEY, name), UNKNOWN_FIELD));
            }
        }
    }
    Ok(())
}

/// Checks that the mounts are the kernel's own, each as it makes it and in its order, then at most one bind mount.
///
/// The kernel makes all of its mounts once each, and the standard makes them in the listed order. A bind mount of a
/// host directory after them is mounted last, as the kernel's file backend.
fn check_mounts(value: &Value) -> Result<Option<Volume>, Error> {
    let kernel = mounts();
    let kernel = kernel.as_array().expect("a list");
    let mut given = value.as_array().map(Vec::as_slice).unwrap_or_default();
    let mut volume = None;
    if given.len() > kernel.len() {
        let index = kernel.len();
        volume = Some(check_bind_mount(index, &given[index])?);
        if given.len() > index + 1 {
            return Err(refused(mount_field(index + 1), ONE_DIRECTORY));
        }
        given = &given[..index];
    }
    let options = |mount: &Value| -> Vec<String> {
        let mut options: Vec<String> = mount[OPTIONS_KEY]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|option| option.as_str().map(str::to_string))
            .collect();
        options.sort();
        options
    };
    let same = |ours: &Value, mount: &Value| {
        ours[DESTINATION_KEY] == mount[DESTINATION_KEY]
            && ours[TYPE_KEY] == mount[TYPE_KEY]
            && ours[SOURCE_KEY] == mount[SOURCE_KEY]
            && options(ours) == options(mount)
    };
    for (index, mount) in given.iter().enumerate() {
        let field = mount_field(index);
        if let Some(name) = mount
            .as_object()
            .into_iter()
            .flatten()
            .map(|(name, _)| name)
            .find(|name| !MOUNT_KEYS.contains(&name.as_str()))
        {
            return Err(refused(
                member(&field, name),
                format!("id-mapped mounts: {NEEDS_USER_NAMESPACES}"),
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
            .any(|mount| mount[DESTINATION_KEY] == ours[DESTINATION_KEY])
        {
            return Err(refused(
                MOUNTS_KEY,
                format!(
                    "lacks {}, which the kernel always mounts; leaving it out needs the kernel to \
                     take its mounts at start",
                    ours[DESTINATION_KEY]
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
            .filter_map(|ours| ours[DESTINATION_KEY].as_str())
            .collect();
        return Err(refused(
            mount_field(index),
            format!(
                "{}: the kernel makes its mounts once each, in the order {}; another order or a \
                 repeat needs the kernel to take its mounts at start",
                given[index],
                order.join(LIST_SEPARATOR)
            ),
        ));
    }
    Ok(volume)
}

/// Returns the bind mount of host directory `source` at `destination`, read and write, as this runtime honours one.
///
/// It is to be added after the configuration's other mounts.
pub fn bind_mount(source: &std::path::Path, destination: &str) -> oci_spec::runtime::Mount {
    let mut mount = oci_spec::runtime::Mount::default();
    mount.set_destination(destination.into());
    mount.set_typ(Some(BIND.to_string()));
    mount.set_source(Some(source.to_path_buf()));
    mount.set_options(Some(vec![RBIND.to_string(), READ_WRITE.to_string()]));
    mount
}

/// Returns the terminal `process.terminal` asks for, of `process.consoleSize`'s size, or 0 by 0 without one.
///
/// A size of 0 by 0 is what `docker run --tty` gives when its client has no terminal; the size is ignored without a
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
                member(&member(PROCESS_KEY, CONSOLE_SIZE_KEY), name),
                format!(
                    "{value}: a terminal's size is at most {} characters",
                    u16::MAX
                ),
            )
        })
    };
    Ok(Some(TerminalSize {
        columns: fit(size.width(), WIDTH_KEY)?,
        rows: fit(size.height(), HEIGHT_KEY)?,
    }))
}

/// Returns the bind mount of a host directory, read and write, at an absolute path other than the root.
fn check_bind_mount(index: usize, mount: &Value) -> Result<Volume, Error> {
    let field = mount_field(index);
    let options: Vec<&str> = mount[OPTIONS_KEY]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    let bind = mount[TYPE_KEY].as_str() == Some(BIND)
        || options.iter().any(|option| BIND_OPTIONS.contains(option));
    if !bind {
        return Err(refused(
            &field,
            format!(
                "{mount}: of the kernel's own mounts and bind mounts of host directories only these \
                 are made; other file systems need the kernel"
            ),
        ));
    }
    if let Some(option) = options
        .iter()
        .find(|option| !BIND_OPTIONS.contains(option) && !HONOURED_OPTIONS.contains(option))
    {
        return Err(refused(
            member(&field, OPTIONS_KEY),
            format!(
                "{option}: a host directory is mounted read and write with no propagation; other \
                 options need the kernel's mounts to take them"
            ),
        ));
    }
    let destination = mount[DESTINATION_KEY].as_str().unwrap_or_default();
    if !destination.starts_with(ABSOLUTE_START)
        || destination.trim_end_matches(ABSOLUTE_START).is_empty()
    {
        return Err(refused(
            member(&field, DESTINATION_KEY),
            format!("{destination:?}: a bind mount needs an absolute path other than the root"),
        ));
    }
    let source = mount[SOURCE_KEY].as_str().unwrap_or_default();
    if !source.starts_with(ABSOLUTE_START) {
        return Err(refused(
            member(&field, SOURCE_KEY),
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
        let path = member(PROCESS_KEY, name);
        match name.as_str() {
            ARGS_KEY | ENV_KEY | CWD_KEY => {}
            TERMINAL_KEY | CONSOLE_SIZE_KEY => {}
            USER_KEY => check_user(field)?,
            COMMAND_LINE_KEY => refuse_unless_absent(&path, field, NEEDS_WINDOWS)?,
            CAPABILITIES_KEY => {
                let sets = field.as_object().cloned().unwrap_or_default();
                if let Some(set) = FILLED_SETS
                    .iter()
                    .copied()
                    .find(|set| !sets.contains_key(*set))
                {
                    return Err(refused(
                        member(&path, set),
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
                    let mut ours: Vec<&str> = if FILLED_SETS.contains(&set.as_str()) {
                        CAPABILITIES.to_vec()
                    } else if EMPTY_SETS.contains(&set.as_str()) {
                        Vec::new()
                    } else {
                        return Err(refused(member(&path, set), UNKNOWN_FIELD));
                    };
                    ours.sort_unstable();
                    if names != ours {
                        return Err(refused(
                            member(&path, set),
                            format!(
                                "{capabilities}: the kernel gives {ours:?}; others need the kernel \
                                 to take capabilities at start"
                            ),
                        ));
                    }
                }
            }
            RLIMITS_KEY => {
                for (index, limit) in field.as_array().into_iter().flatten().enumerate() {
                    let kind = limit[TYPE_KEY].as_str().unwrap_or_default();
                    let (soft, hard) = LIMITS
                        .iter()
                        .find(|(name, ..)| *name == kind)
                        .map_or((u64::MAX, u64::MAX), |&(_, soft, hard)| (soft, hard));
                    if limit[SOFT_KEY].as_u64() != Some(soft)
                        || limit[HARD_KEY].as_u64() != Some(hard)
                    {
                        return Err(refused(
                            element(&path, index),
                            format!(
                                "{limit}: the kernel starts {kind} at soft {soft}, hard {hard}; \
                                 others need the kernel to take limits at start"
                            ),
                        ));
                    }
                }
            }
            NO_NEW_PRIVILEGES_KEY => refuse_unless_absent(&path, field, NEEDS_NO_NEW_PRIVILEGES)?,
            APPARMOR_PROFILE_KEY | SELINUX_LABEL_KEY => {
                refuse_unless_absent(&path, field, NEEDS_SECURITY_MODULE)?;
            }
            OOM_SCORE_ADJ_KEY | IO_PRIORITY_KEY | SCHEDULER_KEY | EXEC_CPU_AFFINITY_KEY => {
                refuse_unless_absent(&path, field, NEEDS_VALUE_AT_START)?;
            }
            _ => return Err(refused(path, UNKNOWN_FIELD)),
        }
    }
    Ok(())
}

fn check_user(value: &Value) -> Result<(), Error> {
    for (name, field) in value.as_object().into_iter().flatten() {
        let path = member(&member(PROCESS_KEY, USER_KEY), name);
        match name.as_str() {
            ADDITIONAL_GIDS_KEY => {}
            UID_KEY | GID_KEY => {
                if field.as_u64() != Some(u64::from(ROOT_ID)) {
                    return Err(refused(
                        path,
                        format!(
                            "{field}: the kernel starts the first process as root; others need it to take the user at start"
                        ),
                    ));
                }
            }
            UMASK_KEY => {
                if field.as_u64() != Some(UMASK) {
                    return Err(refused(
                        path,
                        format!(
                            "{field}: the kernel starts with umask {UMASK:04o}; others need it to take the umask at start"
                        ),
                    ));
                }
            }
            USERNAME_KEY => refuse_unless_absent(&path, field, NEEDS_WINDOWS)?,
            _ => return Err(refused(path, UNKNOWN_FIELD)),
        }
    }
    Ok(())
}

fn check_linux(value: &Value) -> Result<(), Error> {
    for (name, field) in value.as_object().into_iter().flatten() {
        let path = member(LINUX_KEY, name);
        match name.as_str() {
            NAMESPACES_KEY => check_namespaces(field)?,
            UID_MAPPINGS_KEY | GID_MAPPINGS_KEY => {
                refuse_unless_absent(&path, field, NEEDS_USER_NAMESPACES)?
            }
            SYSCTL_KEY => refuse_unless_absent(&path, field, NEEDS_SETTINGS)?,
            RESOURCES_KEY | CGROUPS_PATH_KEY => refuse_unless_absent(&path, field, NEEDS_CGROUPS)?,
            DEVICES_KEY => refuse_unless_absent(&path, field, NEEDS_DEVICES)?,
            SECCOMP_KEY => refuse_unless_absent(&path, field, NEEDS_SECCOMP)?,
            READONLY_PATHS_FIELD => {
                check_kernel_paths(&path, field, &READONLY_PATHS, MAKES_READ_ONLY)?
            }
            MASKED_PATHS_FIELD => check_kernel_paths(&path, field, &MASKED_PATHS, MAKES_MASKED)?,
            ROOTFS_PROPAGATION_KEY | MOUNT_LABEL_KEY => {
                refuse_unless_absent(&path, field, NEEDS_LABELLED_MOUNTS)?
            }
            INTEL_RDT_KEY | MEMORY_POLICY_KEY | PERSONALITY_KEY | TIME_OFFSETS_KEY
            | NET_DEVICES_KEY => {
                refuse_unless_absent(&path, field, NEEDS_VALUE_AT_START)?;
            }
            _ => return Err(refused(path, UNKNOWN_FIELD)),
        }
    }
    Ok(())
}

/// Checks that a list of paths at `path` names exactly those the kernel `makes` read-only or masked, in any order.
fn check_kernel_paths(path: &str, value: &Value, made: &[&str], makes: &str) -> Result<(), Error> {
    let mut given: Vec<&str> = value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    given.sort_unstable();
    let mut made = made.to_vec();
    made.sort_unstable();
    if given != made {
        return Err(refused(
            path,
            if value.is_null() {
                format!("absent: the kernel makes {made:?} {makes}")
            } else {
                format!("{value}: the kernel makes {made:?} {makes}")
            },
        ));
    }
    Ok(())
}

/// Checks that the namespaces list names exactly those the kernel has, none joined by path, as each container has its own.
fn check_namespaces(value: &Value) -> Result<(), Error> {
    let list = member(LINUX_KEY, NAMESPACES_KEY);
    let given = value.as_array().map(Vec::as_slice).unwrap_or_default();
    for (index, namespace) in given.iter().enumerate() {
        let field = element(&list, index);
        let kind = namespace[TYPE_KEY].as_str().unwrap_or_default();
        if !empty(&namespace[PATH_KEY]) {
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
        if !given.iter().any(|namespace| namespace[TYPE_KEY] == kind) {
            return Err(refused(
                list.as_str(),
                format!("lacks {kind}: the container would share the host's, which it never sees"),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Checks that a configuration as `runc spec` writes it, with resources, seccomp and devices added, is read whole.
    ///
    /// Every field is one oci-spec reads, so none is ignored; a property deep in it that the specification does not
    /// define is ignored and named by its path.
    #[test]
    fn a_whole_configuration_is_read_with_nothing_unread() {
        let given = json!({
            OCI_VERSION_KEY: "1.0.2-dev",
            PROCESS_KEY: {
                TERMINAL_KEY: true,
                USER_KEY: { UID_KEY: ROOT_ID, GID_KEY: ROOT_ID },
                ARGS_KEY: ["sh"],
                ENV_KEY: [DEFAULT_PATH, "TERM=xterm"],
                CWD_KEY: ROOT_PATH,
                CAPABILITIES_KEY: {
                    BOUNDING_KEY: ["CAP_AUDIT_WRITE", "CAP_KILL", "CAP_NET_BIND_SERVICE"],
                    EFFECTIVE_KEY: ["CAP_AUDIT_WRITE", "CAP_KILL", "CAP_NET_BIND_SERVICE"],
                    PERMITTED_KEY: ["CAP_AUDIT_WRITE", "CAP_KILL", "CAP_NET_BIND_SERVICE"],
                    AMBIENT_KEY: ["CAP_AUDIT_WRITE", "CAP_KILL", "CAP_NET_BIND_SERVICE"]
                },
                RLIMITS_KEY: [{ TYPE_KEY: "RLIMIT_NOFILE", HARD_KEY: 1024, SOFT_KEY: 1024 }],
                NO_NEW_PRIVILEGES_KEY: true
            },
            ROOT_KEY: { PATH_KEY: "rootfs", READONLY_KEY: true },
            HOSTNAME_KEY: "runc",
            MOUNTS_KEY: [
                { DESTINATION_KEY: "/proc", TYPE_KEY: "proc", SOURCE_KEY: "proc" },
                { DESTINATION_KEY: "/dev", TYPE_KEY: "tmpfs", SOURCE_KEY: "tmpfs",
                  OPTIONS_KEY: ["nosuid", "strictatime", "mode=755", "size=65536k"] }
            ],
            LINUX_KEY: {
                RESOURCES_KEY: {
                    DEVICES_KEY: [{"allow": false, "access": "rwm"}],
                    "cpu": {"shares": 1024},
                    "pids": {"limit": 10}
                },
                NAMESPACES_KEY: NAMESPACES.iter().map(|kind| json!({ TYPE_KEY: kind })).collect::<Vec<_>>(),
                SECCOMP_KEY: {
                    "defaultAction": "SCMP_ACT_ERRNO",
                    "architectures": ["SCMP_ARCH_AARCH64"],
                    "syscalls": [{"names": ["getcwd"], "action": "SCMP_ACT_ALLOW"}]
                },
                MASKED_PATHS_FIELD: ["/proc/acpi", "/proc/kcore"],
                READONLY_PATHS_FIELD: ["/proc/bus", "/proc/sys"]
            }
        });
        let (_, ignored) = spec_from_json(&serde_json::to_vec(&given).unwrap()).unwrap();
        assert_eq!(ignored, Vec::<String>::new());
        let mut unknown = given;
        unknown[LINUX_KEY][RESOURCES_KEY]["cpu"]["sharez"] = json!(1);
        let (_, ignored) = spec_from_json(&serde_json::to_vec(&unknown).unwrap()).unwrap();
        assert_eq!(ignored, ["linux.resources.cpu.sharez"]);
    }
}
