//! What a container's configuration sets at its start beyond its program (the user and group,
//! the umask, resource limits, no_new_privs, the `oom_score_adj`, the domain name, capabilities,
//! seccomp, scheduling, the I/O priority and the personality), compared with the reference given
//! the same through Docker's options, or through the reference shell where Docker has none.

use core::mem::offset_of;

use linux_raw_sys::general::RLIMIT_NOFILE;
use wcontainers_kernel::linux::general::{__NR_mkdirat, CAP_CHOWN, CAP_NET_ADMIN, SCHED_NORMAL};
use wcontainers_kernel::linux::ptrace::{
    AUDIT_ARCH_AARCH64, BPF_ABS, BPF_JEQ, BPF_JMP, BPF_K, BPF_LD, BPF_RET, BPF_W,
    SECCOMP_RET_ALLOW, SECCOMP_RET_ERRNO, SECCOMP_RET_KILL_PROCESS, seccomp_data, sock_filter,
};
use wcontainers_kernel::{
    ADDR_NO_RANDOMIZE, CapabilitySets, DOCKER_CAPABILITIES, DOCKER_LIMITS, DOCKER_UMASK, Errno,
    IOPRIO_CLASS_BE, IOPRIO_CLASS_SHIFT, Seccomp, StartOptions, sched_attr,
};
use wcontainers_kernel_test::busybox::run_ours_with;
use wcontainers_kernel_test::reference::{self, docker};

/// What the first process shows of its start: ids, mask, limits, powers, scores and names, and
/// the mode a new file gets.
const SHOW: &str = "id; umask; ulimit -n; ulimit -Hn; ulimit -s; ulimit -Hs; \
    grep -E '^(NoNewPrivs|Cap(Inh|Prm|Eff|Bnd|Amb))' /proc/self/status; \
    cat /proc/sys/kernel/domainname; cat /proc/self/oom_score_adj; \
    echo x > /tmp/made; stat -c '%a %u %g' /tmp/made; \
    echo -1000 > /proc/self/oom_score_adj; echo lower $?";

/// A resource limit in Docker's option form.
fn ulimit(name: &str, soft: u64, hard: u64) -> String {
    format!(
        "{name}{}{soft}{}{hard}",
        docker::ULIMIT_SEPARATOR,
        docker::FIELD_SEPARATOR
    )
}

/// Docker's limits with those given in place of their defaults.
fn limits(given: &[(u32, u64, u64)]) -> &'static [(u32, u64, u64)] {
    let mut limits: Vec<(u32, u64, u64)> = DOCKER_LIMITS
        .iter()
        .copied()
        .filter(|(resource, _, _)| given.iter().all(|(other, _, _)| other != resource))
        .collect();
    limits.extend_from_slice(given);
    limits.leak()
}

fn compare(options: StartOptions<'static>, docker_options: &[&str], umask: &str) {
    // Docker cannot set the mask: the reference's shell sets it first.
    compare_shown(
        options,
        docker_options,
        &format!("umask {umask}; {SHOW}"),
        SHOW,
    );
}

/// Compares what `show` prints on ours started with `options` with what `theirs_script` prints in
/// a reference container made with `docker_options`.
fn compare_shown(
    options: StartOptions<'static>,
    docker_options: &[&str],
    theirs_script: &str,
    show: &str,
) {
    let theirs = reference::run_command_with(docker_options, &["sh", "-c", theirs_script]);
    let ours = run_ours_with(&["sh", "-c", show], options);
    assert_eq!(
        String::from_utf8_lossy(&ours.outcome.stdout),
        String::from_utf8_lossy(&theirs.stdout),
        "{docker_options:?}: standard output"
    );
    assert_eq!(
        String::from_utf8_lossy(&ours.outcome.stderr),
        String::from_utf8_lossy(&theirs.stderr),
        "{docker_options:?}: standard error"
    );
    assert_eq!(
        ours.outcome.exit_code, theirs.exit_code,
        "{docker_options:?}"
    );
}

#[test]
fn defaults_match_reference() {
    compare(StartOptions::default(), &[], &format!("{DOCKER_UMASK:o}"));
}

#[test]
fn a_user_with_limits_without_new_privileges_matches_reference() {
    let (user, group) = (1234, 1235);
    let (nofile_soft, nofile_hard) = (100, 200);
    let (oom_score_adj, domain_name) = (321, "example.test");
    let identity = format!("{user}{}{group}", docker::FIELD_SEPARATOR);
    let nofile = ulimit(docker::ULIMIT_NOFILE, nofile_soft, nofile_hard);
    let score = oom_score_adj.to_string();
    let docker_options = [
        docker::USER_OPTION,
        &identity,
        docker::ULIMIT_OPTION,
        &nofile,
        docker::SECURITY_OPTION,
        docker::NO_NEW_PRIVILEGES,
        docker::DOMAIN_NAME_OPTION,
        domain_name,
        docker::OOM_SCORE_ADJ_OPTION,
        &score,
    ];
    let umask = 0o027;
    let options = StartOptions {
        user,
        group,
        umask,
        limits: limits(&[(RLIMIT_NOFILE, nofile_soft, nofile_hard)]),
        no_new_privileges: true,
        oom_score_adj,
        domain_name: domain_name.as_bytes(),
        ..StartOptions::default()
    };
    compare(options, &docker_options, &format!("{umask:o}"));
}

/// Root in the container with only some limits and a mask changed, as a runtime gives a
/// configuration that names no user.
#[test]
fn root_with_a_mask_and_a_limit_matches_reference() {
    let (nofile_soft, nofile_hard) = (64, 64);
    let nofile = ulimit(docker::ULIMIT_NOFILE, nofile_soft, nofile_hard);
    let umask = 0o077;
    let options = StartOptions {
        umask,
        limits: limits(&[(RLIMIT_NOFILE, nofile_soft, nofile_hard)]),
        ..StartOptions::default()
    };
    compare(
        options,
        &[docker::ULIMIT_OPTION, &nofile],
        &format!("{umask:o}"),
    );
}

/// Capability sets as Docker gives them for `--cap-add` and `--cap-drop`: the same bounding,
/// effective and permitted sets, none inherited or ambient.
fn docker_sets(capabilities: u64) -> CapabilitySets {
    CapabilitySets {
        effective: capabilities,
        permitted: capabilities,
        inheritable: 0,
        bounding: capabilities,
        ambient: 0,
    }
}

#[test]
fn added_and_dropped_capabilities_match_reference() {
    let capabilities = DOCKER_CAPABILITIES & !(1 << CAP_CHOWN) | 1 << CAP_NET_ADMIN;
    let options = StartOptions {
        capabilities: docker_sets(capabilities),
        ..StartOptions::default()
    };
    let docker_options = [
        docker::CAP_DROP_OPTION,
        "CHOWN",
        docker::CAP_ADD_OPTION,
        "NET_ADMIN",
    ];
    let show = format!("{SHOW}; chown 1 /tmp/made; echo chown $?");
    let theirs = format!("umask {DOCKER_UMASK:o}; {show}");
    compare_shown(options, &docker_options, &theirs, &show);
}

/// What the scheduling values show: the seccomp mode, the personality, the nice value, the I/O
/// priority, and the nice value and policy in `stat`.
const SHOW_SCHEDULING: &str = "grep -E \"^Seccomp\" /proc/self/status; cat /proc/self/personality; \
    nice; ionice -p $$; cut -d\" \" -f19,41 /proc/self/stat";

/// Without seccomp, which lets the reference's shell set a personality Docker's profile refuses;
/// Docker has no option for these values, so that shell sets them before its program runs, as
/// runc would.
#[test]
fn unconfined_with_a_personality_a_nice_value_and_an_io_priority_matches_reference() {
    let (nice, io_level) = (5, 6);
    let options = StartOptions {
        seccomp: Seccomp::Unconfined,
        scheduler: Some(sched_attr {
            size: size_of::<sched_attr>() as u32,
            sched_policy: SCHED_NORMAL,
            sched_nice: nice,
            ..Default::default()
        }),
        io_priority: IOPRIO_CLASS_BE << IOPRIO_CLASS_SHIFT | io_level,
        personality: ADDR_NO_RANDOMIZE,
        ..StartOptions::default()
    };
    let theirs = format!(
        "renice -n {nice} -p $$; ionice -c {IOPRIO_CLASS_BE} -n {io_level} -p $$; \
         exec linux64 -R sh -c '{SHOW_SCHEDULING}'"
    );
    compare_shown(
        options,
        &[docker::SECURITY_OPTION, docker::SECCOMP_UNCONFINED],
        &theirs,
        SHOW_SCHEDULING,
    );
}

/// A filter of the container's own, here one that refuses `mkdirat` with EACCES on ARM64 and
/// ends a process calling as another architecture, as libseccomp builds it from the profile.
fn mkdirat_refused() -> &'static [sock_filter] {
    let load = (BPF_LD | BPF_W | BPF_ABS) as u16;
    let equal = (BPF_JMP | BPF_JEQ | BPF_K) as u16;
    let answer = (BPF_RET | BPF_K) as u16;
    let at = |field: usize| field as u32;
    let program = vec![
        sock_filter {
            code: load,
            jt: 0,
            jf: 0,
            k: at(offset_of!(seccomp_data, arch)),
        },
        sock_filter {
            code: equal,
            jt: 1,
            jf: 0,
            k: AUDIT_ARCH_AARCH64,
        },
        sock_filter {
            code: answer,
            jt: 0,
            jf: 0,
            k: SECCOMP_RET_KILL_PROCESS,
        },
        sock_filter {
            code: load,
            jt: 0,
            jf: 0,
            k: at(offset_of!(seccomp_data, nr)),
        },
        sock_filter {
            code: equal,
            jt: 0,
            jf: 1,
            k: __NR_mkdirat,
        },
        sock_filter {
            code: answer,
            jt: 0,
            jf: 0,
            k: SECCOMP_RET_ERRNO | Errno::EACCES.0,
        },
        sock_filter {
            code: answer,
            jt: 0,
            jf: 0,
            k: SECCOMP_RET_ALLOW,
        },
    ];
    program.leak()
}

#[test]
fn a_seccomp_profile_of_the_containers_own_matches_reference() {
    let profile = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("mkdirat-refused.json");
    let json = format!(
        "{{\"defaultAction\":\"SCMP_ACT_ALLOW\",\"syscalls\":[{{\"names\":[\"mkdirat\"],\
         \"action\":\"SCMP_ACT_ERRNO\",\"errnoRet\":{}}}]}}",
        Errno::EACCES.0
    );
    std::fs::write(&profile, json).expect("writing the profile");
    let security = format!("{}{}", docker::SECCOMP_PROFILE, profile.display());
    let options = StartOptions {
        seccomp: Seccomp::Filter(mkdirat_refused()),
        ..StartOptions::default()
    };
    let show = "grep -E '^Seccomp' /proc/self/status; mkdir /tmp/made; echo mkdir $?; \
        grep -E '^NoNewPrivs' /proc/self/status";
    compare_shown(options, &[docker::SECURITY_OPTION, &security], show, show);
}
