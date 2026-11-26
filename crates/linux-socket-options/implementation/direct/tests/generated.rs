//! Makes the trait and the direct implementation from `options.table` and fails when the committed files differ.
//!
//! Run with `REGENERATE=1` to write them instead.

use std::fmt::Write as _;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The variable that makes the test write the files instead of comparing them.
const REGENERATE: &str = "REGENERATE";

/// The columns of a row of the table: method, level, option, value, manual page, what the kernel does with the memory.
const COLUMNS: usize = 6;

/// What the table's comment lines start with.
const TABLE_COMMENT: char = '#';

/// The level column of an ioctl request, which has none.
const NO_LEVEL: &str = "-";

/// One option or request of the table.
struct Row<'a> {
    method: &'a str,
    level: &'a str,
    option: &'a str,
    value: &'a str,
    manual: &'a str,
    kernel: &'a str,
}

impl Row<'_> {
    fn sets(&self) -> bool {
        self.method.starts_with(SET_PREFIX)
    }

    /// Tells whether the kernel may write into the value a set gives it.
    fn written_back(&self) -> bool {
        self.kernel == WRITES_BACK
    }
}

/// The kernel column of a set whose value the kernel only reads.
const READS: &str = "reads";

/// The kernel column of a set whose value the kernel may write into.
const WRITES_BACK: &str = "writes-back";

/// The kernel column of a get that writes at most the length it is told.
const WITHIN: &str = "within";

/// The kernel column of a get that may write past the length it is told.
const PAST: &str = "past";

/// The kernel column of a get that also writes at the addresses its value holds.
const THROUGH: &str = "through";

/// Every kernel column the table may name.
const KERNELS: [&str; 5] = [READS, WRITES_BACK, WITHIN, PAST, THROUGH];

/// What a method's name starts with: a set writes the option, a get reads it.
const SET_PREFIX: &str = "set_";
const GET_PREFIX: &str = "get_";

/// The forms whose name is followed by a type, as the table's comment lists them.
const STRUCT_FORM: &str = "struct:";
const SLICE_FORM: &str = "slice:";

/// Each form and the kernel columns its generated code honors, for a set and for a get.
///
/// A set of a number or flag copies the value, so only `reads` fits; a structure is borrowed shared or exclusive as the
/// column says; bytes and runs are borrowed shared, as no option the kernel writes back takes them and the probe has no
/// exclusive form for them; `program` and `group_filter` are written into memory the method owns and only read.
/// A get of a number, a room or an ioctl tells the kernel exactly the room it owns, so only `within` fits; `keys` limits
/// the answers asked to the one its room holds, so only `past` needs it; `full_info` hands the kernel the addresses of
/// rooms it owns, so only `through` needs it. Every pair not listed is refused.
const SET_FORMS: [(&str, &[&str]); 9] = [
    ("bool", &[READS]),
    ("c_int", &[READS]),
    ("u32", &[READS]),
    ("u64", &[READS]),
    ("bytes", &[READS]),
    (STRUCT_FORM, &[READS, WRITES_BACK]),
    (SLICE_FORM, &[READS]),
    ("program", &[READS]),
    ("group_filter", &[READS]),
];
const GET_FORMS: [(&str, &[&str]); 7] = [
    ("c_int", &[WITHIN]),
    ("u32", &[WITHIN]),
    ("u64", &[WITHIN]),
    ("room", &[WITHIN]),
    ("ioctl", &[WITHIN]),
    ("keys", &[PAST]),
    ("full_info", &[THROUGH]),
];

/// The forms whose generated code knows the layout of one option's value, and the only options each may carry.
///
/// Each of these options, in turn, may take no other form: given as bytes or as another structure, its value would
/// let the caller hand the kernel an address (`sock_fprog`, `mptcp_full_info`) or ask past the room (`TCP_AO_GET_KEYS`).
const BOUND_FORMS: [(&str, &[&str]); 5] = [
    ("program", &["SO_ATTACH_FILTER", "SO_ATTACH_REUSEPORT_CBPF"]),
    ("group_filter", &["MCAST_MSFILTER"]),
    ("keys", &["TCP_AO_GET_KEYS"]),
    ("full_info", &["MPTCP_FULL_INFO"]),
    ("ioctl", &["SIOCOUTQ", "SIOCOUTQNSD", "SIOCATMARK"]),
];

/// The uapi structures a `struct:` or `slice:` value may name: each holds numbers, bytes and addresses of the network
/// only, no address of the caller's memory the kernel would follow, so it may be marked `Plain`.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/tcp.h
const PLAIN_STRUCTURES: [&str; 11] = [
    "uapi::so_timestamping",
    "uapi::group_req",
    "uapi::in6_flowlabel_req",
    "uapi::ipv6_mreq",
    "uapi::tcp_md5sig",
    "uapi::tcp_ao_add",
    "uapi::tcp_ao_del",
    "uapi::tcp_ao_info_opt",
    "uapi::tcp_ao_repair",
    "uapi::tcp_repair_window",
    "uapi::tcp_repair_opt",
];

/// The options a `bytes` value may set: each takes an opaque run of bytes (IP options, IPv6 extension headers, a key)
/// and reads no address from it.
const BYTE_OPTIONS: [&str; 6] = [
    "IP_OPTIONS",
    "IPV6_HOPOPTS",
    "IPV6_RTHDRDSTOPTS",
    "IPV6_RTHDR",
    "IPV6_DSTOPTS",
    "TCP_FASTOPEN_KEY",
];

/// Gives the form of a value, `struct:` or `slice:` without its type.
fn form(value: &str) -> &str {
    [STRUCT_FORM, SLICE_FORM]
        .into_iter()
        .find(|prefix| value.starts_with(prefix))
        .unwrap_or(value)
}

/// Gives why the generator refuses a row, or nothing when its form, column, level and option fit together.
fn refusal(row: &Row<'_>) -> Option<String> {
    let method = row.method;
    let sets = row.method.starts_with(SET_PREFIX);
    if !sets && !row.method.starts_with(GET_PREFIX) {
        return Some(format!(
            "{method}: a method is named {SET_PREFIX}* or {GET_PREFIX}*"
        ));
    }
    if !KERNELS.contains(&row.kernel) {
        return Some(format!("{method}: no kernel column {}", row.kernel));
    }
    let shape = form(row.value);
    let forms: &[(&str, &[&str])] = if sets { &SET_FORMS } else { &GET_FORMS };
    let Some((_, columns)) = forms.iter().find(|(name, _)| *name == shape) else {
        let direction = if sets { "set" } else { "get" };
        return Some(format!(
            "{method}: no form for a {direction} of {}",
            row.value
        ));
    };
    if !columns.contains(&row.kernel) {
        return Some(format!(
            "{method}: a {shape} value cannot be {}",
            row.kernel
        ));
    }
    if (row.level == NO_LEVEL) != (shape == "ioctl") {
        return Some(format!(
            "{method}: an ioctl request has level {NO_LEVEL}, and only it"
        ));
    }
    let option = bare(row.option);
    for (bound, options) in BOUND_FORMS {
        if (shape == bound) != options.contains(&option) {
            return Some(format!(
                "{method}: the {bound} form carries only {options:?}, and they no other form"
            ));
        }
    }
    if matches!(shape, STRUCT_FORM | SLICE_FORM) {
        let structure = &row.value[shape.len()..];
        if !PLAIN_STRUCTURES.contains(&structure) {
            return Some(format!(
                "{method}: {structure} is not a structure known to hold no address"
            ));
        }
    }
    if shape == "bytes" && !BYTE_OPTIONS.contains(&option) {
        return Some(format!(
            "{method}: {option} is not an option known to read an opaque run of bytes"
        ));
    }
    None
}

/// The probe's name for each kernel column, as `probe.rs` declares `Kernel`.
fn kernel_variant(kernel: &str) -> &'static str {
    match kernel {
        READS => "Reads",
        WRITES_BACK => "WritesBack",
        WITHIN => "Within",
        PAST => "Past",
        THROUGH => "Through",
        other => panic!("no kernel column {other}"),
    }
}

/// Gives the last part of a path, the name the manual pages use.
fn bare(path: &str) -> &str {
    path.rsplit("::").next().unwrap_or(path)
}

/// Gives the interface's folder, which holds the table, the trait and the suite, two above this crate's.
fn crate_folder() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn rows(table: &str) -> Vec<Row<'_>> {
    table
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with(TABLE_COMMENT))
        .map(|line| {
            let columns: Vec<&str> = line.split_whitespace().collect();
            assert_eq!(columns.len(), COLUMNS, "a row of {COLUMNS} columns: {line}");
            Row {
                method: columns[0],
                level: columns[1],
                option: columns[2],
                value: columns[3],
                manual: columns[4],
                kernel: columns[5],
            }
        })
        .inspect(|row| {
            if let Some(reason) = refusal(row) {
                panic!("options.table refused: {reason}");
            }
        })
        .collect()
}

/// Gives the reference a set takes its value by: shared when the kernel only reads it, exclusive when it may write.
fn reference(shared: bool) -> &'static str {
    if shared { "&" } else { "&mut " }
}

/// The parameters after `socket` and the type of the value a method gives, as the trait declares them.
fn signature(row: &Row<'_>) -> (String, String, String) {
    let unit = "()".to_owned();
    let shared = !row.written_back();
    let (generics, parameters, result) = match (row.sets(), row.value) {
        (true, value)
            if !shared
                && !(value.starts_with("struct:")
                    || value.starts_with("slice:")
                    || value == "bytes") =>
        {
            panic!(
                "{}: a value the kernel writes into is a structure, a run or bytes, taken by exclusive reference",
                row.method
            )
        }
        (true, "bool") => (String::new(), ", on: bool".to_owned(), unit),
        (true, number @ ("c_int" | "u32" | "u64")) => {
            (String::new(), format!(", value: {number}"), unit)
        }
        (true, "bytes") => (
            String::new(),
            format!(", value: {}[u8]", reference(shared)),
            unit,
        ),
        (true, "program") => (
            String::new(),
            ", program: &[libc::sock_filter]".to_owned(),
            unit,
        ),
        (true, "group_filter") => (String::new(), ", filter: &GroupFilter<'_>".to_owned(), unit),
        (true, value) if value.starts_with("struct:") => (
            String::new(),
            format!(
                ", value: {}{}",
                reference(shared),
                &value["struct:".len()..]
            ),
            unit,
        ),
        (true, value) if value.starts_with("slice:") => (
            String::new(),
            format!(
                ", value: {}[{}]",
                reference(shared),
                &value["slice:".len()..]
            ),
            unit,
        ),
        (false, number @ ("c_int" | "u32" | "u64")) => {
            (String::new(), String::new(), number.to_owned())
        }
        (false, "ioctl") => (String::new(), String::new(), "c_int".to_owned()),
        (false, "room") => (
            "<const ROOM: usize>".to_owned(),
            String::new(),
            "Written<ROOM>".to_owned(),
        ),
        (false, "keys") => (
            String::new(),
            ", query: &[u8; KEY_QUERY_LENGTH]".to_owned(),
            "Written<KEY_QUERY_LENGTH>".to_owned(),
        ),
        (false, "full_info") => (
            "<const SUBFLOW_ROOM: usize, const INFORMATION_ROOM: usize>".to_owned(),
            ", subflows: usize".to_owned(),
            "MptcpFullInformation<SUBFLOW_ROOM, INFORMATION_ROOM>".to_owned(),
        ),
        (sets, value) => panic!(
            "no form for a {} of {value}",
            if sets { "set" } else { "get" }
        ),
    };
    (generics, parameters, result)
}

/// The method's documentation, one sentence naming the option and the manual page that defines it.
fn summary(row: &Row<'_>) -> String {
    let option = bare(row.option);
    let manual = row.manual;
    if row.level == NO_LEVEL {
        format!("Reads the int of the `{option}` request ({manual}).")
    } else if row.written_back() {
        format!("Sets `{option}` to a value the kernel may write into ({manual}).")
    } else if row.sets() {
        format!("Sets `{option}` ({manual}).")
    } else {
        format!("Reads `{option}` ({manual}).")
    }
}

/// The body of the direct implementation's method.
fn body(row: &Row<'_>) -> String {
    let (level, option) = (row.level, row.option);
    match (row.sets(), row.value) {
        (true, "bool") => format!("set_number(socket, {level}, {option}, &c_int::from(on))"),
        (true, "c_int" | "u32" | "u64") => format!("set_number(socket, {level}, {option}, &value)"),
        (true, "program") => format!("set_program(socket, {level}, {option}, program)"),
        (true, "group_filter") => format!("set_group_filter(socket, {level}, {option}, filter)"),
        (true, _) if row.written_back() => {
            format!("set_value_written(socket, {level}, {option}, value)")
        }
        (true, _) => format!("set_value(socket, {level}, {option}, value)"),
        (false, number @ ("c_int" | "u32" | "u64")) => {
            format!("get_value::<{number}>(socket, {level}, {option})")
        }
        (false, "ioctl") => format!("ioctl_int(socket, {option})"),
        (false, "room") => format!("get_written(socket, {level}, {option})"),
        (false, "keys") => format!("get_keys(socket, {level}, {option}, query)"),
        (false, "full_info") => format!("get_full_info(socket, {level}, {option}, subflows)"),
        (_, value) => panic!("{}: no body for {value}", row.method),
    }
}

const INTERFACE_HEAD: &str = r#"//! Linux's socket option and socket ioctl calls that no vetted crate wraps safely, one method each.
//!
//! The calls are setsockopt(2), getsockopt(2) and ioctl(2) on a socket. Made from `options.table` by the direct
//! implementation's `tests/generated.rs`; edit the table, not this file.
#![cfg(target_os = "linux")]

use std::ffi::c_int;
use std::mem::size_of;
use std::os::fd::BorrowedFd;

use linux_raw_sys::net as uapi;
use wcontainers_abi::HostError;
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

/// The bytes a read wrote into a room of `ROOM` bytes, the rest zero, and how many it wrote.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Written<const ROOM: usize> {
    pub bytes: [u8; ROOM],
    pub length: usize,
}

/// The length of the kernel's socket address storage, `struct __kernel_sockaddr_storage`.
pub const SOCKET_STORAGE_LENGTH: usize = size_of::<uapi::__kernel_sockaddr_storage>();

/// A socket address in the kernel's storage form, as bytes aligned as `struct __kernel_sockaddr_storage` is.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/socket.h
#[repr(C, align(8))]
#[derive(Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
pub struct SocketStorage {
    pub bytes: [u8; SOCKET_STORAGE_LENGTH],
}

/// A multicast group's source filter, as `MCAST_MSFILTER` takes it in `struct group_filter`.
pub struct GroupFilter<'a> {
    pub interface: u32,
    pub group: SocketStorage,
    /// `MCAST_INCLUDE` or `MCAST_EXCLUDE`.
    pub mode: u32,
    pub sources: &'a [SocketStorage],
}

/// The length of `struct mptcp_info` as Linux 6.18 defines it, which `MPTCP_FULL_INFO` holds.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/mptcp.h
pub const MPTCP_INFO_LENGTH: usize = 96;

/// The length of one `struct tcp_ao_getsockopt`, the query and answer of `TCP_AO_GET_KEYS`.
pub const KEY_QUERY_LENGTH: usize = size_of::<uapi::tcp_ao_getsockopt>();

/// What `MPTCP_FULL_INFO` gave: the kernel's lengths and counts, the connection's information and each subflow's.
///
/// Each subflow's `struct mptcp_subflow_info` and `struct tcp_info` sit in rooms of the sizes asked, as many as asked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MptcpFullInformation<const SUBFLOW_ROOM: usize, const INFORMATION_ROOM: usize> {
    /// `size_tcpinfo_kernel`, the length of the kernel's `struct tcp_info`.
    pub tcp_information_length: u32,
    /// `size_sfinfo_kernel`, the length of the kernel's `struct mptcp_subflow_info`.
    pub subflow_information_length: u32,
    /// `num_subflows`, every subflow of the connection, also those past the rooms.
    pub subflows: u32,
    pub connection: [u8; MPTCP_INFO_LENGTH],
    pub subflow_information: Vec<[u8; SUBFLOW_ROOM]>,
    pub tcp_information: Vec<[u8; INFORMATION_ROOM]>,
}

#[allow(
    reviewable::explicit_shared_error::HostError,
    reason = "each method is one system call on the host's socket and fails only the ways that call does"
)]
pub trait LinuxSocketOptions {
"#;

const DIRECT_HEAD: &str = "//! The trait's methods as the calls they name, made from the interface's `options.table`.\n\n";

/// The names the direct implementation's methods may use, from the interface and from the crate's own calls.
const INTERFACE_NAMES: [&str; 5] = [
    "GroupFilter",
    "KEY_QUERY_LENGTH",
    "LinuxSocketOptions",
    "MptcpFullInformation",
    "Written",
];
const CRATE_NAMES: [&str; 13] = [
    "DirectLinuxSocketOptions",
    "Plain",
    "get_full_info",
    "get_keys",
    "get_value",
    "get_written",
    "ioctl_int",
    "local",
    "set_group_filter",
    "set_number",
    "set_program",
    "set_value",
    "set_value_written",
];

/// Gives the names of the list that the code uses, as a `use` list.
fn used(names: &[&str], code: &str) -> String {
    let words: Vec<&str> = code
        .split(|character: char| !(character.is_alphanumeric() || character == '_'))
        .collect();
    names
        .iter()
        .copied()
        .filter(|name| words.contains(name))
        .collect::<Vec<_>>()
        .join(", ")
}

fn interface(rows: &[Row<'_>]) -> String {
    let mut text = INTERFACE_HEAD.to_owned();
    for (index, row) in rows.iter().enumerate() {
        if index > 0 {
            text.push('\n');
        }
        let (generics, parameters, result) = signature(row);
        writeln!(text, "    /// {}", summary(row)).unwrap();
        writeln!(
            text,
            "    fn {}{generics}(&self, socket: BorrowedFd<'_>{parameters}) -> Result<{result}, HostError>;",
            row.method
        )
        .unwrap();
    }
    text.push_str("}\n");
    text
}

/// The table's rows as the probe walks them, so that a row added to the table is probed or the probe fails.
fn probe_rows(rows: &[Row<'_>]) -> String {
    let mut code = format!(
        "\n/// The rows of `options.table`, which the probe calls each of.\n#[cfg(test)]\npub(crate) const ROWS: [crate::probe::Row; {}] = [\n",
        rows.len()
    );
    for row in rows {
        writeln!(
            code,
            "    crate::probe::Row {{ method: \"{}\", value: \"{}\", kernel: crate::probe::Kernel::{} }},",
            row.method,
            row.value,
            kernel_variant(row.kernel)
        )
        .unwrap();
    }
    code.push_str("];\n");
    code
}

fn direct(rows: &[Row<'_>]) -> String {
    let mut code = String::new();
    let mut plain: Vec<&str> = rows
        .iter()
        .filter_map(|row| {
            row.value
                .strip_prefix("struct:")
                .or_else(|| row.value.strip_prefix("slice:"))
        })
        .collect();
    plain.sort_unstable();
    plain.dedup();
    for name in &plain {
        writeln!(code, "impl Plain for {name} {{}}").unwrap();
    }
    if !plain.is_empty() {
        code.push('\n');
    }
    code.push_str("impl LinuxSocketOptions for DirectLinuxSocketOptions {\n");
    for (index, row) in rows.iter().enumerate() {
        if index > 0 {
            code.push('\n');
        }
        let (generics, parameters, result) = signature(row);
        writeln!(
            code,
            "    fn {}{generics}(&self, socket: BorrowedFd<'_>{parameters}) -> Result<{result}, HostError> {{",
            row.method
        )
        .unwrap();
        writeln!(code, "        {}", body(row)).unwrap();
        code.push_str("    }\n");
    }
    code.push_str("}\n");
    code.push_str(&probe_rows(rows));
    let mut text = DIRECT_HEAD.to_owned();
    if used(&["c_int"], &code).is_empty() {
        text.push_str("use std::os::fd::BorrowedFd;\n\n");
    } else {
        text.push_str("use std::ffi::c_int;\nuse std::os::fd::BorrowedFd;\n\n");
    }
    text.push_str("use linux_raw_sys::net as uapi;\nuse wcontainers_abi::HostError;\n");
    writeln!(
        text,
        "use wcontainers_linux_socket_options::{{{}}};\n",
        used(&INTERFACE_NAMES, &code)
    )
    .unwrap();
    writeln!(text, "use crate::{{{}}};\n", used(&CRATE_NAMES, &code)).unwrap();
    text.push_str(&code);
    text
}

/// Gives the text as rustfmt lays it out, so that the committed files pass `cargo fmt --check`.
fn formatted(made: &str) -> String {
    let mut rustfmt = Command::new("rustfmt")
        .args(["--edition", "2024", "--emit", "stdout", "--quiet"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("rustfmt runs");
    rustfmt
        .stdin
        .take()
        .expect("rustfmt's input")
        .write_all(made.as_bytes())
        .expect("rustfmt takes the text");
    let output = rustfmt.wait_with_output().expect("rustfmt ends");
    assert!(output.status.success(), "rustfmt lays the text out");
    String::from_utf8(output.stdout).expect("rustfmt gives text")
}

/// Compares a made file with the committed one, or writes it when asked to.
fn check(path: &Path, made: &str) {
    let made = formatted(made);
    if std::env::var_os(REGENERATE).is_some() {
        fs::write(path, made).expect("the generated file is written");
        return;
    }
    let committed = fs::read_to_string(path).expect("the generated file is there");
    assert!(
        committed == made,
        "{} differs from what options.table gives: run this test with {REGENERATE}=1",
        path.display()
    );
}

#[test]
fn the_trait_and_the_direct_implementation_are_what_the_table_gives() {
    let folder = crate_folder();
    let table = fs::read_to_string(folder.join("options.table")).expect("the table is there");
    let rows = rows(&table);
    check(&folder.join("interface.rs"), &interface(&rows));
    check(
        &folder.join("implementation/direct/generated.rs"),
        &direct(&rows),
    );
}

#[test]
fn every_method_is_listed_once() {
    let table =
        fs::read_to_string(crate_folder().join("options.table")).expect("the table is there");
    let mut methods: Vec<&str> = rows(&table).iter().map(|row| row.method).collect();
    let listed = methods.len();
    methods.sort_unstable();
    methods.dedup();
    assert_eq!(methods.len(), listed, "a method listed twice");
}

#[test]
fn the_suite_calls_every_method() {
    let folder = crate_folder();
    let table = fs::read_to_string(folder.join("options.table")).expect("the table is there");
    let suite =
        fs::read_to_string(folder.join("conformance/suite.rs")).expect("the suite is there");
    let missing: Vec<&str> = rows(&table)
        .iter()
        .map(|row| row.method)
        .filter(|method| {
            !suite.contains(&format!(".{method}(")) && !suite.contains(&format!(".{method}::"))
        })
        .collect();
    assert!(missing.is_empty(), "the suite calls no {missing:?}");
}

#[test]
fn the_probe_calls_every_method() {
    let folder = crate_folder();
    let table = fs::read_to_string(folder.join("options.table")).expect("the table is there");
    let probe = fs::read_to_string(folder.join("implementation/direct/probe.rs"))
        .expect("the probe is there");
    let missing: Vec<&str> = rows(&table)
        .iter()
        .map(|row| row.method)
        .filter(|method| {
            !probe
                .split("row!(")
                .skip(1)
                .any(|rest| rest.trim_start().starts_with(&format!("{method},")))
        })
        .collect();
    assert!(missing.is_empty(), "the probe has no case for {missing:?}");
}

/// A row of a planted table: the method, the level and option a form needs, the value and the kernel column.
fn planted<'a>(
    method: &'a str,
    level: &'a str,
    option: &'a str,
    value: &'a str,
    kernel: &'a str,
) -> Row<'a> {
    Row {
        method,
        level,
        option,
        value,
        manual: "planted(7)",
        kernel,
    }
}

/// One value of each form, with the level and option it is generated for when it fits its column.
const PLANTED_VALUES: [(&str, &str, &str); 13] = [
    ("bool", "uapi::SOL_SOCKET", "uapi::SO_RCVMARK"),
    ("c_int", "uapi::SOL_SOCKET", "uapi::SO_RCVLOWAT"),
    ("u32", "uapi::SOL_TCP", "uapi::TCP_QUEUE_SEQ"),
    ("u64", "uapi::SOL_SOCKET", "uapi::SO_MAX_PACING_RATE"),
    ("bytes", "uapi::SOL_IP", "uapi::IP_OPTIONS"),
    ("room", "uapi::SOL_TCP", "uapi::TCP_INFO"),
    (
        "struct:uapi::group_req",
        "uapi::SOL_IP",
        "uapi::MCAST_JOIN_GROUP",
    ),
    (
        "slice:uapi::tcp_repair_opt",
        "uapi::SOL_TCP",
        "uapi::TCP_REPAIR_OPTIONS",
    ),
    (
        "program",
        "uapi::SOL_SOCKET",
        "uapi::SO_ATTACH_REUSEPORT_CBPF",
    ),
    ("group_filter", "uapi::SOL_IP", "uapi::MCAST_MSFILTER"),
    ("keys", "uapi::SOL_TCP", "uapi::TCP_AO_GET_KEYS"),
    ("full_info", "uapi::SOL_MPTCP", "local::MPTCP_FULL_INFO"),
    ("ioctl", NO_LEVEL, "local::SIOCOUTQ"),
];

/// Every (direction, form, column) the generator takes, written out apart from the code it checks.
const TAKEN: [(&str, &str, &str); 17] = [
    (GET_PREFIX, "ioctl", WITHIN),
    (SET_PREFIX, "bool", READS),
    (SET_PREFIX, "c_int", READS),
    (SET_PREFIX, "u32", READS),
    (SET_PREFIX, "u64", READS),
    (SET_PREFIX, "bytes", READS),
    (SET_PREFIX, "struct:uapi::group_req", READS),
    (SET_PREFIX, "struct:uapi::group_req", WRITES_BACK),
    (SET_PREFIX, "slice:uapi::tcp_repair_opt", READS),
    (SET_PREFIX, "program", READS),
    (SET_PREFIX, "group_filter", READS),
    (GET_PREFIX, "c_int", WITHIN),
    (GET_PREFIX, "u32", WITHIN),
    (GET_PREFIX, "u64", WITHIN),
    (GET_PREFIX, "room", WITHIN),
    (GET_PREFIX, "keys", PAST),
    (GET_PREFIX, "full_info", THROUGH),
];

#[test]
fn every_pair_of_form_and_column_outside_the_list_is_refused() {
    let mut refused = 0;
    for direction in [SET_PREFIX, GET_PREFIX] {
        for (value, level, option) in PLANTED_VALUES {
            for kernel in KERNELS {
                let method = format!("{direction}planted");
                let row = planted(&method, level, option, value, kernel);
                let taken = TAKEN.contains(&(direction, value, kernel));
                let reason = refusal(&row);
                assert_eq!(
                    reason.is_none(),
                    taken,
                    "{direction}{value} {kernel}: {reason:?}"
                );
                refused += usize::from(reason.is_some());
            }
        }
    }
    let pairs = [SET_PREFIX, GET_PREFIX].len() * PLANTED_VALUES.len() * KERNELS.len();
    assert_eq!(refused, pairs - TAKEN.len());
}

#[test]
fn the_planted_room_read_marked_past_is_refused() {
    let row = planted(
        "get_tcp_ao_keys_room",
        "uapi::SOL_TCP",
        "uapi::TCP_AO_GET_KEYS",
        "room",
        PAST,
    );
    assert!(refusal(&row).is_some());
}

#[test]
fn a_level_and_a_request_that_do_not_fit_are_refused() {
    // An ioctl form given a level, and a request without one in another form, would ignore or misplace it.
    let rows = [
        planted(
            "get_planted",
            "uapi::SOL_TCP",
            "local::SIOCOUTQ",
            "ioctl",
            WITHIN,
        ),
        planted("get_planted", NO_LEVEL, "uapi::TCP_INFO", "room", WITHIN),
        planted("set_planted", NO_LEVEL, "uapi::SO_RCVMARK", "bool", READS),
        planted(
            "planted",
            "uapi::SOL_SOCKET",
            "uapi::SO_RCVMARK",
            "bool",
            READS,
        ),
        planted(
            "set_planted",
            "uapi::SOL_SOCKET",
            "uapi::SO_RCVMARK",
            "bool",
            "copies",
        ),
    ];
    for row in rows {
        assert!(
            refusal(&row).is_some(),
            "{} {} {}",
            row.level,
            row.option,
            row.value
        );
    }
}

#[test]
fn an_option_bound_to_a_form_takes_no_other_and_the_form_no_other_option() {
    let rows = [
        // A program given as bytes or as a structure: the caller would write the address `sock_fprog` holds.
        planted(
            "set_planted",
            "uapi::SOL_SOCKET",
            "uapi::SO_ATTACH_REUSEPORT_CBPF",
            "bytes",
            READS,
        ),
        planted(
            "set_planted",
            "uapi::SOL_SOCKET",
            "uapi::SO_ATTACH_FILTER",
            "struct:uapi::group_req",
            READS,
        ),
        // The full information read as a room: the kernel would follow the addresses in the room's bytes.
        planted(
            "get_planted",
            "uapi::SOL_MPTCP",
            "local::MPTCP_FULL_INFO",
            "room",
            WITHIN,
        ),
        // The key query read as a number, and the full information form on the key option.
        planted(
            "get_planted",
            "uapi::SOL_TCP",
            "uapi::TCP_AO_GET_KEYS",
            "c_int",
            WITHIN,
        ),
        planted(
            "get_planted",
            "uapi::SOL_TCP",
            "uapi::TCP_AO_GET_KEYS",
            "full_info",
            THROUGH,
        ),
        // Each bound form on an option whose layout it does not know.
        planted(
            "set_planted",
            "uapi::SOL_SOCKET",
            "uapi::SO_RCVMARK",
            "program",
            READS,
        ),
        planted(
            "set_planted",
            "uapi::SOL_IP",
            "uapi::MCAST_JOIN_GROUP",
            "group_filter",
            READS,
        ),
        planted(
            "get_planted",
            "uapi::SOL_TCP",
            "uapi::TCP_INFO",
            "keys",
            PAST,
        ),
        planted(
            "get_planted",
            "uapi::SOL_MPTCP",
            "local::MPTCP_INFO",
            "full_info",
            THROUGH,
        ),
        planted(
            "get_planted",
            NO_LEVEL,
            "local::SIOCGSTAMP",
            "ioctl",
            WITHIN,
        ),
    ];
    for row in rows {
        assert!(refusal(&row).is_some(), "{} {}", row.option, row.value);
    }
}

#[test]
fn a_structure_or_bytes_not_known_to_hold_no_address_is_refused() {
    let rows = [
        planted(
            "set_planted",
            "uapi::SOL_SOCKET",
            "uapi::SO_RCVMARK",
            "struct:libc::sock_fprog",
            READS,
        ),
        planted(
            "set_planted",
            "uapi::SOL_TCP",
            "uapi::TCP_REPAIR_OPTIONS",
            "slice:libc::iovec",
            READS,
        ),
        planted(
            "set_planted",
            "uapi::SOL_SOCKET",
            "uapi::SO_BINDTODEVICE",
            "bytes",
            READS,
        ),
    ];
    for row in rows {
        assert!(refusal(&row).is_some(), "{} {}", row.option, row.value);
    }
}

#[test]
fn every_row_of_the_table_is_taken() {
    let table =
        fs::read_to_string(crate_folder().join("options.table")).expect("the table is there");
    let refused: Vec<String> = table
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with(TABLE_COMMENT))
        .filter_map(|line| {
            let columns: Vec<&str> = line.split_whitespace().collect();
            refusal(&planted(
                columns[0], columns[1], columns[2], columns[3], columns[5],
            ))
        })
        .collect();
    assert!(refused.is_empty(), "{refused:?}");
}
