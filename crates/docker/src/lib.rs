//! The Docker client and the words of its command line.

/// The client's program name.
pub const CLIENT: &str = "docker";
/// Creates and starts a container.
pub const START_CONTAINER: &str = "run";
/// Writes a container's files as a tar archive.
pub const EXPORT: &str = "export";
/// Removes a container.
pub const REMOVE: &str = "rm";
/// With [`REMOVE`], stops a running container first.
pub const FORCE: &str = "--force";
/// With [`START_CONTAINER`], removes the container when it exits.
pub const REMOVE_ON_EXIT: &str = "--rm";
/// The platform option.
pub const PLATFORM_OPTION: &str = "--platform";
/// The container name option.
pub const NAME_OPTION: &str = "--name";
/// The host name option.
pub const HOSTNAME_OPTION: &str = "--hostname";
/// The bind mount option.
pub const VOLUME_OPTION: &str = "--volume";
/// Between the fields of a bind mount or a published port.
pub const FIELD_SEPARATOR: &str = ":";
/// A bind mount's option for read-only.
pub const READ_ONLY: &str = "ro";
/// Keeps standard input open, from the client's.
pub const INTERACTIVE: &str = "-i";
/// Runs a command in a running container.
pub const EXEC: &str = "exec";
/// Gives the container a terminal.
pub const TERMINAL: &str = "-t";
/// Publishes a container port.
pub const PUBLISH_OPTION: &str = "--publish";
/// The first process's user and group, as `user:group`.
pub const USER_OPTION: &str = "--user";
/// A resource limit, as `name=soft:hard`.
pub const ULIMIT_OPTION: &str = "--ulimit";
/// Between a name and its value in Docker's options (`--ulimit`, `--sysctl`).
pub const VALUE_SEPARATOR: &str = "=";
/// The open files limit's name.
pub const ULIMIT_NOFILE: &str = "nofile";
/// A security option.
pub const SECURITY_OPTION: &str = "--security-opt";
/// The security option that sets no_new_privs.
pub const NO_NEW_PRIVILEGES: &str = "no-new-privileges";
/// The NIS domain name option.
pub const DOMAIN_NAME_OPTION: &str = "--domainname";
/// The first process's `oom_score_adj` option.
pub const OOM_SCORE_ADJ_OPTION: &str = "--oom-score-adj";
/// Adds a capability, by its name without `CAP_`.
pub const CAP_ADD_OPTION: &str = "--cap-add";
/// Drops a capability, likewise.
pub const CAP_DROP_OPTION: &str = "--cap-drop";
/// Sets a sysctl, as `key=value`.
pub const SYSCTL_OPTION: &str = "--sysctl";
/// The security option that filters no call.
pub const SECCOMP_UNCONFINED: &str = "seccomp=unconfined";
/// Before the path of a seccomp profile, as a security option.
pub const SECCOMP_PROFILE: &str = "seccomp=";
