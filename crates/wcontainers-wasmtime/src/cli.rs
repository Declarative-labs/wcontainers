//! The options of the command line program `wcon`, as `docker run` names those it shares.

pub const OCI: &str = "--oci";
pub const TAR: &str = "--tar";
pub const CONFIG: &str = "--config";
pub const CACHE: &str = "--cache";
pub const LOOK_AHEAD: &str = "--look-ahead";
pub const VOLUME: &str = "--volume";
pub const PUBLISH: &str = "--publish";
pub const INTERACTIVE: &str = "--interactive";
pub const TTY: &str = "--tty";
pub const SIZE: &str = "--size";
/// What ends the options: the arguments after it are `process.args`.
pub const END_OF_OPTIONS: &str = "--";
