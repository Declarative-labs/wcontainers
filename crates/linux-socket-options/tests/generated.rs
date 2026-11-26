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
const COMMENT: char = '#';

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
        self.method.starts_with("set_")
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

/// The kernel columns of a get: at most the length told, past it, and also through the addresses the value holds.
const GET_KERNELS: [&str; 3] = ["within", "past", "through"];

/// The probe's name for each kernel column, as `probe.rs` declares `Kernel`.
fn kernel_variant(kernel: &str) -> &'static str {
    match kernel {
        READS => "Reads",
        WRITES_BACK => "WritesBack",
        "within" => "Within",
        "past" => "Past",
        "through" => "Through",
        other => panic!("no kernel column {other}"),
    }
}

/// Gives the last part of a path, the name the manual pages use.
fn bare(path: &str) -> &str {
    path.rsplit("::").next().unwrap_or(path)
}

fn crate_folder() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn rows(table: &str) -> Vec<Row<'_>> {
    table
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with(COMMENT))
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
            let allowed = if row.sets() {
                [READS, WRITES_BACK].contains(&row.kernel)
            } else {
                GET_KERNELS.contains(&row.kernel)
            };
            assert!(allowed, "{} cannot be {}", row.method, row.kernel);
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

/// The method's documentation, one sentence.
fn summary(row: &Row<'_>) -> String {
    let option = bare(row.option);
    let manual = row.manual;
    if row.level == NO_LEVEL {
        return format!(
            "Reads the int the `{option}` ioctl request gives for the socket, as {manual} defines it."
        );
    }
    let level = bare(row.level);
    let what = match row.value {
        "bool" => "on or off",
        "c_int" | "u32" | "u64" => "a number",
        "bytes" => "a byte string",
        "program" => "a classic BPF program",
        "group_filter" => "a source filter",
        _ if row.value.starts_with("slice:") => "a run of values",
        _ => "a value",
    };
    if row.written_back() {
        format!(
            "Sets `{option}` of level `{level}` to {what} the kernel may write into, as {manual} defines it."
        )
    } else if row.sets() {
        format!("Sets `{option}` of level `{level}` to {what}, as {manual} defines it.")
    } else {
        format!("Reads `{option}` of level `{level}`, as {manual} defines it.")
    }
}

/// The body of the direct implementation's method.
fn body(row: &Row<'_>) -> String {
    let (level, option) = (row.level, row.option);
    match (row.sets(), row.value) {
        (true, "bool") => format!("set_value(socket, {level}, {option}, &c_int::from(on))"),
        (true, "c_int" | "u32" | "u64") => format!("set_value(socket, {level}, {option}, &value)"),
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
        (false, _) => format!("get_full_info(socket, {level}, {option}, subflows)"),
    }
}

const INTERFACE_HEAD: &str = r#"//! Linux's socket option and socket ioctl calls (setsockopt(2), getsockopt(2), ioctl(2) on a socket), one method per
//! option and request the host socket crates use that no vetted crate wraps safely.
//!
//! Made from `options.table` by `tests/generated.rs`; edit the table, not this file.
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
#[repr(C)]
#[derive(Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
pub struct SocketStorage {
    _aligned: [usize; 0],
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

/// What `MPTCP_FULL_INFO` gave: the kernel's lengths and counts, the connection's `struct mptcp_info`, and each subflow's
/// `struct mptcp_subflow_info` and `struct tcp_info` in rooms of the sizes asked, as many as asked for.
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
const CRATE_NAMES: [&str; 12] = [
    "DirectLinuxSocketOptions",
    "Plain",
    "get_full_info",
    "get_keys",
    "get_value",
    "get_written",
    "ioctl_int",
    "local",
    "set_group_filter",
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
        let level = if row.level == NO_LEVEL {
            "None".to_owned()
        } else {
            format!("Some({})", row.level)
        };
        writeln!(
            code,
            "    crate::probe::Row {{ method: \"{}\", level: {level}, option: {}, value: \"{}\", kernel: crate::probe::Kernel::{} }},",
            row.method,
            row.option,
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
        .filter(|method| !probe.contains(&format!("\"{method}\",")))
        .collect();
    assert!(missing.is_empty(), "the probe has no case for {missing:?}");
}
