//! What a container's configuration sets at its start beyond its program (the user and group,
//! the umask, resource limits, no_new_privs, the `oom_score_adj` and the domain name), compared
//! with the reference given the same through Docker's options.

use linux_raw_sys::general::RLIMIT_NOFILE;
use wcontainers_kernel::{DOCKER_LIMITS, DOCKER_UMASK, StartOptions};
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
    let theirs_script = format!("umask {umask}; {SHOW}");
    let theirs = reference::run_command_with(docker_options, &["sh", "-c", &theirs_script]);
    let ours = run_ours_with(&["sh", "-c", SHOW], options);
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
