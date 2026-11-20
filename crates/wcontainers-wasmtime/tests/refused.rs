//! Every field of the OCI runtime configuration the runtime cannot honour makes the container fail to
//! start with an error naming the field: one test for each, from the runtime's base configuration
//! with that one field set, read from its JSON. The base configuration itself is accepted.

use std::path::PathBuf;
use std::sync::OnceLock;

use serde_json::{Value, json};
use wcontainers_arm64::Arm64;
use wcontainers_runtime::{
    Error, GuestMachine, Image, LocalSystem, Runtime, RuntimeSettings, base_spec, spec_from_json,
};
use wcontainers_wasmtime::{CpuSide, LookAhead, Optimise, TranslatorSettings};

/// A root holding only its own directory.
fn image() -> &'static PathBuf {
    static TAR: OnceLock<PathBuf> = OnceLock::new();
    TAR.get_or_init(|| {
        let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("wc-refused-root-{}.tar", std::process::id()));
        let mut builder = tar::Builder::new(std::fs::File::create(&path).unwrap());
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(tar::EntryType::Directory);
        header.set_mode(0o755);
        header.set_size(0);
        header.set_uid(0);
        header.set_gid(0);
        header.set_mtime(0);
        builder
            .append_data(&mut header, "./", std::io::empty())
            .unwrap();
        builder.finish().unwrap();
        path
    })
}

fn base() -> Value {
    let mut value = serde_json::to_value(base_spec()).unwrap();
    value["process"]["args"] = json!(["/bin/sh"]);
    value
}

/// Makes a container from the configuration; the properties it ignored.
fn make(value: Value) -> Result<Vec<String>, Error> {
    let (spec, ignored) = spec_from_json(&serde_json::to_vec(&value).unwrap())?;
    let runtime = Runtime::<CpuSide<Arm64>>::new(&RuntimeSettings::default());
    let system = LocalSystem::new(Box::new(std::io::sink()), Box::new(std::io::sink()));
    let translator_settings = TranslatorSettings {
        look_ahead: LookAhead::Full,
        optimise: Optimise::Speed,
    };
    let image = Image::tar(image()).unwrap();
    runtime
        .container(
            &spec,
            &image,
            &translator_settings,
            &GuestMachine { offer: () },
            system,
        )
        .map(|_| ignored)
}

fn refused(field: &str, change: impl FnOnce(&mut Value)) {
    let mut value = base();
    change(&mut value);
    match make(value) {
        Err(Error::Refused {
            field: named,
            reason,
        }) => {
            assert_eq!(named, field, "refused for: {reason}");
            println!("{field}: {reason}");
        }
        Err(other) => panic!("{field}: {other}"),
        Ok(_) => panic!("{field} was accepted"),
    }
}

#[test]
fn the_base_configuration_is_accepted() {
    assert_eq!(make(base()).unwrap(), Vec::<String>::new());
}

/// A property the specification does not define is ignored, and named for a warning.
fn ignored(path: &str, change: impl FnOnce(&mut Value)) {
    let mut value = base();
    change(&mut value);
    match make(value) {
        Ok(ignored) => assert_eq!(ignored, [path]),
        Err(error) => panic!("{path}: {error}"),
    }
}

macro_rules! ignored {
    ($($name:ident: $path:literal, |$value:ident| $change:expr;)*) => {
        $(
            #[test]
            fn $name() {
                ignored($path, |$value: &mut Value| {
                    $change;
                });
            }
        )*
    };
}

ignored! {
    a_field_oci_spec_does_not_read: "hostnmae", |v| v["hostnmae"] = json!("typo");
    a_process_field_oci_spec_does_not_read: "process.futureProcessField", |v| v["process"]
        ["futureProcessField"] = json!(true);
    a_linux_field_oci_spec_does_not_read: "linux.unknownLinuxThing", |v| v["linux"]
        ["unknownLinuxThing"] = json!({"x": 1});
    a_mount_field_oci_spec_does_not_read: "mounts[2].label", |v| v["mounts"][2]["label"] =
        json!("x");
    an_annotation: "annotations.org.example.note", |v| v["annotations"] =
        json!({"org.example.note": "x"});
}

macro_rules! refused {
    ($($name:ident: $field:literal, |$value:ident| $change:expr;)*) => {
        $(
            #[test]
            fn $name() {
                refused($field, |$value: &mut Value| {
                    $change;
                });
            }
        )*
    };
}

refused! {
    oci_version: "ociVersion", |v| v["ociVersion"] = json!("2.0.0");
    root_path: "root.path", |v| v["root"] = json!({"path": "rootfs"});
    root_readonly: "root.readonly", |v| v["root"] = json!({"path": "", "readonly": true});
    uid_mappings: "uidMappings", |v| v["uidMappings"] =
        json!([{"containerID": 0, "hostID": 1000, "size": 1}]);
    gid_mappings: "gidMappings", |v| v["gidMappings"] =
        json!([{"containerID": 0, "hostID": 1000, "size": 1}]);
    a_gid_mapped_mount: "mounts[0].gidMappings", |v| v["mounts"][0]["gidMappings"] =
        json!([{"containerID": 0, "hostID": 1000, "size": 1}]);
    a_bind_mount: "mounts[6]", |v| v["mounts"].as_array_mut().unwrap().push(
        json!({"destination": "/data", "type": "bind", "source": "/srv", "options": ["rbind"]}));
    a_mount_of_another_file_system: "mounts[6]", |v| v["mounts"].as_array_mut().unwrap().push(
        json!({"destination": "/sys/fs/cgroup", "type": "cgroup", "source": "cgroup",
               "options": ["ro", "nosuid", "noexec", "nodev"]}));
    a_mount_with_other_options: "mounts[0]", |v| v["mounts"][0]["options"] = json!(["nosuid"]);
    mounts_absent: "mounts", |v| { v.as_object_mut().unwrap().remove("mounts"); };
    linux_absent: "linux.namespaces", |v| { v.as_object_mut().unwrap().remove("linux"); };
    a_kernel_mount_left_out: "mounts", |v| { v["mounts"].as_array_mut().unwrap().remove(3); };
    kernel_mounts_in_another_order: "mounts[1]", |v| v["mounts"].as_array_mut().unwrap().swap(1, 2);
    a_kernel_mount_repeated: "mounts[6]", |v| {
        let first = v["mounts"][0].clone();
        v["mounts"].as_array_mut().unwrap().push(first);
    };
    an_id_mapped_mount: "mounts[0].uidMappings", |v| v["mounts"][0]["uidMappings"] =
        json!([{"containerID": 0, "hostID": 1000, "size": 1}]);
    process_absent: "process", |v| { v.as_object_mut().unwrap().remove("process"); };
    process_args_empty: "process.args", |v| v["process"]["args"] = json!([]);
    process_cwd_relative: "process.cwd", |v| v["process"]["cwd"] = json!("tmp");
    process_terminal: "process.terminal", |v| v["process"]["terminal"] = json!(true);
    process_console_size: "process.consoleSize", |v| v["process"]["consoleSize"] =
        json!({"height": 24, "width": 80});
    process_user_uid: "process.user.uid", |v| v["process"]["user"]["uid"] = json!(1000);
    process_user_gid: "process.user.gid", |v| v["process"]["user"]["gid"] = json!(1000);
    process_user_umask: "process.user.umask", |v| v["process"]["user"]["umask"] = json!(0o077);
    process_user_username: "process.user.username", |v| v["process"]["user"]["username"] =
        json!("someone");
    process_command_line: "process.commandLine", |v| v["process"]["commandLine"] = json!("sh");
    process_capabilities_bounding: "process.capabilities.bounding", |v| v["process"]
        ["capabilities"]["bounding"].as_array_mut().unwrap().push(json!("CAP_SYS_ADMIN"));
    process_capabilities_ambient: "process.capabilities.ambient", |v| v["process"]
        ["capabilities"]["ambient"] = json!(["CAP_KILL"]);
    process_capabilities_absent: "process.capabilities", |v| {
        v["process"].as_object_mut().unwrap().remove("capabilities"); };
    process_capabilities_effective_absent: "process.capabilities.effective", |v| {
        v["process"]["capabilities"].as_object_mut().unwrap().remove("effective"); };
    process_capabilities_permitted_absent: "process.capabilities.permitted", |v| {
        v["process"]["capabilities"].as_object_mut().unwrap().remove("permitted"); };
    process_capabilities_bounding_absent: "process.capabilities.bounding", |v| {
        v["process"]["capabilities"].as_object_mut().unwrap().remove("bounding"); };
    process_capabilities_inheritable: "process.capabilities.inheritable", |v| v["process"]
        ["capabilities"]["inheritable"] = json!(["CAP_KILL"]);
    process_rlimits: "process.rlimits[0]", |v| v["process"]["rlimits"] =
        json!([{"type": "RLIMIT_NOFILE", "hard": 1024, "soft": 1024}]);
    process_no_new_privileges: "process.noNewPrivileges", |v| v["process"]["noNewPrivileges"] =
        json!(true);
    process_apparmor_profile: "process.apparmorProfile", |v| v["process"]["apparmorProfile"] =
        json!("docker-default");
    process_selinux_label: "process.selinuxLabel", |v| v["process"]["selinuxLabel"] =
        json!("system_u:system_r:container_t:s0");
    process_oom_score_adj: "process.oomScoreAdj", |v| v["process"]["oomScoreAdj"] = json!(100);
    process_io_priority: "process.ioPriority", |v| v["process"]["ioPriority"] =
        json!({"class": "IOPRIO_CLASS_BE", "priority": 4});
    process_scheduler: "process.scheduler", |v| v["process"]["scheduler"] =
        json!({"policy": "SCHED_BATCH"});
    process_exec_cpu_affinity: "process.execCPUAffinity", |v| v["process"]["execCPUAffinity"] =
        json!({"initial": "0"});
    domainname: "domainname", |v| v["domainname"] = json!("example.org");
    hooks: "hooks", |v| v["hooks"] = json!({"prestart": [{"path": "/usr/bin/true"}]});
    linux_uid_mappings: "linux.uidMappings", |v| v["linux"]["uidMappings"] =
        json!([{"containerID": 0, "hostID": 1000, "size": 1}]);
    linux_gid_mappings: "linux.gidMappings", |v| v["linux"]["gidMappings"] =
        json!([{"containerID": 0, "hostID": 1000, "size": 1}]);
    linux_sysctl: "linux.sysctl", |v| v["linux"]["sysctl"] = json!({"net.ipv4.ip_forward": "1"});
    linux_resources: "linux.resources", |v| v["linux"]["resources"] = json!({"pids": {"limit": 10}});
    linux_cgroups_path: "linux.cgroupsPath", |v| v["linux"]["cgroupsPath"] = json!("/wc/one");
    linux_devices: "linux.devices", |v| v["linux"]["devices"] =
        json!([{"path": "/dev/fuse", "type": "c", "major": 10, "minor": 229}]);
    linux_seccomp: "linux.seccomp", |v| v["linux"]["seccomp"] =
        json!({"defaultAction": "SCMP_ACT_ALLOW"});
    linux_rootfs_propagation: "linux.rootfsPropagation", |v| v["linux"]["rootfsPropagation"] =
        json!("private");
    linux_mount_label: "linux.mountLabel", |v| v["linux"]["mountLabel"] =
        json!("system_u:object_r:container_file_t:s0");
    linux_masked_paths: "linux.maskedPaths", |v| v["linux"]["maskedPaths"] = json!(["/proc/kcore"]);
    linux_readonly_paths: "linux.readonlyPaths", |v| v["linux"]["readonlyPaths"] =
        json!(["/proc/sys", "/proc/bus"]);
    linux_readonly_paths_absent: "linux.readonlyPaths", |v| {
        v["linux"].as_object_mut().unwrap().remove("readonlyPaths"); };
    linux_intel_rdt: "linux.intelRdt", |v| v["linux"]["intelRdt"] = json!({"closID": "one"});
    linux_memory_policy: "linux.memoryPolicy", |v| v["linux"]["memoryPolicy"] =
        json!({"mode": "MPOL_BIND", "nodes": "0"});
    linux_personality: "linux.personality", |v| v["linux"]["personality"] =
        json!({"domain": "LINUX32"});
    linux_time_offsets: "linux.timeOffsets", |v| v["linux"]["timeOffsets"] =
        json!({"monotonic": {"secs": 1, "nanosecs": 0}});
    linux_net_devices: "linux.netDevices", |v| v["linux"]["netDevices"] = json!({"eth1": {}});
    a_namespace_joined_by_path: "linux.namespaces[0]", |v| v["linux"]["namespaces"][0]["path"] =
        json!("/proc/1/ns/pid");
    a_user_namespace: "linux.namespaces[6]", |v| v["linux"]["namespaces"].as_array_mut().unwrap()
        .push(json!({"type": "user"}));
    a_time_namespace: "linux.namespaces[6]", |v| v["linux"]["namespaces"].as_array_mut().unwrap()
        .push(json!({"type": "time"}));
    a_namespace_left_out: "linux.namespaces", |v| { v["linux"]["namespaces"].as_array_mut()
        .unwrap().remove(0); };
    solaris: "solaris", |v| v["solaris"] = json!({"milestone": "svc:/milestone/container:default"});
    windows: "windows", |v| v["windows"] = json!({"layerFolders": ["C:\\layers"]});
    vm: "vm", |v| v["vm"] = json!({"kernel": {"path": "/boot/vmlinuz"}});
    zos: "zos", |v| v["zos"] = json!({"namespaces": [{"type": "pid"}]});
}
