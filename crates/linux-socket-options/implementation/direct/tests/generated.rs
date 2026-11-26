//! Makes the trait and the direct implementation from `options.table` and fails when the committed files differ.
//!
//! Run with `REGENERATE=1` to write them instead.

use std::fmt::Write as _;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use kernel::{Does, Entry, Families};

#[path = "generated/kernel.rs"]
mod kernel;

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

/// The forms a set may take, and those a get may take, as the table's comment lists them.
const SET_FORMS: [&str; 9] = [
    "bool",
    "c_int",
    "u32",
    "u64",
    "bytes",
    STRUCT_FORM,
    SLICE_FORM,
    "program",
    "group_filter",
];
const GET_FORMS: [&str; 7] = ["c_int", "u32", "u64", "room", "ioctl", "keys", "full_info"];

/// The uapi structures a `struct:` or `slice:` value may name, so that each may be marked `Plain`.
///
/// Each holds numbers, bytes and addresses of the network only, no address of the caller's memory the kernel would
/// follow; which of them the kernel reads at a pair is the kernel table's.
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

/// Gives the form of a value, `struct:` or `slice:` without its type.
fn form(value: &str) -> &str {
    [STRUCT_FORM, SLICE_FORM]
        .into_iter()
        .find(|prefix| value.starts_with(prefix))
        .unwrap_or(value)
}

/// Gives the uapi type a `struct:` or `slice:` value names.
fn named_type(value: &str) -> Option<&str> {
    value
        .strip_prefix(STRUCT_FORM)
        .or_else(|| value.strip_prefix(SLICE_FORM))
}

/// Tells whether a name is a Rust identifier of lower-case letters, digits and underscores, as a method's must be.
fn identifier(name: &str) -> bool {
    !name.is_empty()
        && name.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
}

/// Gives the entry of the kernel table the row's level and option names resolve to, by their numbers.
fn entry(row: &Row<'_>) -> Result<&'static Entry, String> {
    let level = (row.level != NO_LEVEL).then_some(row.level);
    kernel::resolve(level, row.option).map_err(|reason| format!("{}: {reason}", row.method))
}

/// Gives what the kernel does at the row's pair in the row's direction.
fn does(row: &Row<'_>, entry: &Entry) -> Does {
    if row.sets() { entry.set } else { entry.get }
}

/// Tells whether the code the generator makes for the row's form is sound where the kernel does `does`.
///
/// A number is copied whole into a room of the method's own; bytes, a structure and a run are lent whole, and only
/// where the kernel reads exactly that type; a structure the kernel writes into is lent from an exclusive borrow; a
/// program and a source filter are written by the method in the layout the kernel reads; a get's room is the length
/// told; a key query asks at most the answers its room holds; the full information rooms are the method's own; an
/// ioctl's room holds one int.
fn fits(row: &Row<'_>, does: Does) -> bool {
    let value = row.value;
    match (row.sets(), form(value), does) {
        (true, "bool" | "c_int" | "u32" | "u64", Does::ReadsNumber)
        | (true, "bytes", Does::ReadsBytes)
        | (true, "program", Does::FollowsProgram)
        | (true, "group_filter", Does::ReadsGroupFilter)
        | (false, "c_int" | "u32" | "u64" | "room", Does::WritesWithin)
        | (false, "keys", Does::WritesPerKey)
        | (false, "full_info", Does::WritesThroughFullInfo)
        | (false, "ioctl", Does::WritesInt) => true,
        (true, STRUCT_FORM, Does::ReadsStructure(read)) => {
            row.kernel == READS && named_type(value) == Some(read)
        }
        (true, STRUCT_FORM, Does::WritesBack(written)) => {
            row.kernel == WRITES_BACK && named_type(value) == Some(written)
        }
        (true, SLICE_FORM, Does::ReadsRun(read)) => named_type(value) == Some(read),
        _ => false,
    }
}

/// Gives the kernel column that names what the kernel does, or nothing for what no row may take.
fn column(does: Does) -> Option<&'static str> {
    match does {
        Does::ReadsNumber
        | Does::ReadsBytes
        | Does::ReadsStructure(_)
        | Does::ReadsRun(_)
        | Does::ReadsGroupFilter
        | Does::FollowsProgram => Some(READS),
        Does::WritesBack(_) => Some(WRITES_BACK),
        Does::WritesWithin | Does::WritesInt => Some(WITHIN),
        Does::WritesPerKey => Some(PAST),
        Does::WritesThroughFullInfo => Some(THROUGH),
        Does::Unknown | Does::Follows | Does::WritesPast | Does::WritesStructure(_) => None,
    }
}

/// Gives why the generator refuses a row, or nothing when its names resolve and its form fits what the kernel does.
fn refusal(row: &Row<'_>) -> Option<String> {
    let method = row.method;
    let sets = row.method.starts_with(SET_PREFIX);
    if !identifier(method) || !(sets || row.method.starts_with(GET_PREFIX)) {
        return Some(format!(
            "{method}: a method is an identifier named {SET_PREFIX}* or {GET_PREFIX}*"
        ));
    }
    if !KERNELS.contains(&row.kernel) {
        return Some(format!("{method}: no kernel column {}", row.kernel));
    }
    let shape = form(row.value);
    let forms: &[&str] = if sets { &SET_FORMS } else { &GET_FORMS };
    if !forms.contains(&shape) {
        let direction = if sets { "set" } else { "get" };
        return Some(format!(
            "{method}: no form for a {direction} of {}",
            row.value
        ));
    }
    if (row.level == NO_LEVEL) != (shape == "ioctl") {
        return Some(format!(
            "{method}: an ioctl request has level {NO_LEVEL}, and only it"
        ));
    }
    if let Some(structure) = named_type(row.value)
        && !PLAIN_STRUCTURES.contains(&structure)
    {
        return Some(format!(
            "{method}: {structure} is not a structure known to hold no address"
        ));
    }
    let entry = match entry(row) {
        Ok(entry) => entry,
        Err(reason) => return Some(reason),
    };
    let does = does(row, entry);
    if column(does) != Some(row.kernel) {
        return Some(format!(
            "{method}: the kernel table says {does:?} at this pair, which is not the column {}",
            row.kernel
        ));
    }
    if !fits(row, does) {
        return Some(format!(
            "{method}: a {} value does not fit what the kernel does at this pair, {does:?}",
            row.value
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

/// The private functions that check a socket's family, for an option and for an ioctl request.
const OPTION_FAMILY_CHECK: &str = "of_family";
const REQUEST_FAMILY_CHECK: &str = "request_of_family";

/// Gives the check of the socket's family a generated call passes the socket through, from the kernel table's families
/// of the row's level; nothing where the level is the socket layer's own, which every family shares.
fn family_guard(row: &Row<'_>) -> Option<String> {
    let level = (row.level != NO_LEVEL).then_some(row.level);
    match kernel::families(level)? {
        Families::Every => None,
        Families::Only(names) => {
            let list: Vec<&str> = names.iter().map(|name| name.path).collect();
            let check = if level.is_none() {
                REQUEST_FAMILY_CHECK
            } else {
                OPTION_FAMILY_CHECK
            };
            Some(format!("{check}(socket, &[{}])?", list.join(", ")))
        }
    }
}

/// The body of the direct implementation's method.
fn body(row: &Row<'_>) -> String {
    let (level, option) = (row.level, row.option);
    let socket = family_guard(row).unwrap_or_else(|| "socket".to_owned());
    match (row.sets(), row.value) {
        (true, "bool") => format!("set_number({socket}, {level}, {option}, &c_int::from(on))"),
        (true, "c_int" | "u32" | "u64") => {
            format!("set_number({socket}, {level}, {option}, &value)")
        }
        (true, "program") => format!("set_program({socket}, {level}, {option}, program)"),
        (true, "group_filter") => format!("set_group_filter({socket}, {level}, {option}, filter)"),
        (true, _) if row.written_back() => {
            format!("set_value_written({socket}, {level}, {option}, value)")
        }
        (true, _) => format!("set_value({socket}, {level}, {option}, value)"),
        (false, number @ ("c_int" | "u32" | "u64")) => {
            format!("get_value::<{number}>({socket}, {level}, {option})")
        }
        (false, "ioctl") => format!("ioctl_int({socket}, {option})"),
        (false, "room") => format!("get_written({socket}, {level}, {option})"),
        (false, "keys") => format!("get_keys({socket}, {level}, {option}, query)"),
        (false, "full_info") => format!("get_full_info({socket}, {level}, {option}, subflows)"),
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
const CRATE_NAMES: [&str; 15] = [
    "DirectLinuxSocketOptions",
    "Plain",
    "get_full_info",
    "get_keys",
    "get_value",
    "get_written",
    "ioctl_int",
    "local",
    "of_family",
    "request_of_family",
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
            "    crate::probe::Row {{ method: \"{}\", value: \"{}\", kernel: crate::probe::Kernel::{}, level: {}, option: {} }},",
            row.method,
            row.value,
            kernel_variant(row.kernel),
            if row.level == NO_LEVEL {
                "None".to_owned()
            } else {
                format!("Some({})", row.level)
            },
            row.option
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
const PLANTED_VALUES: [(&str, &str, &str); 14] = [
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
        "struct:uapi::in6_flowlabel_req",
        "uapi::SOL_IPV6",
        "uapi::IPV6_FLOWLABEL_MGR",
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

/// Every (direction, form, column) the generator takes at those pairs, written out apart from the code it checks.
const TAKEN: [(&str, &str, &str); 17] = [
    (GET_PREFIX, "ioctl", WITHIN),
    (SET_PREFIX, "bool", READS),
    (SET_PREFIX, "c_int", READS),
    (SET_PREFIX, "u32", READS),
    (SET_PREFIX, "u64", READS),
    (SET_PREFIX, "bytes", READS),
    (SET_PREFIX, "struct:uapi::group_req", READS),
    (SET_PREFIX, "struct:uapi::in6_flowlabel_req", WRITES_BACK),
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

// The kernel table.

#[test]
fn the_kernel_table_holds_each_pair_once_and_every_name_resolves_to_an_entry() {
    let mut pairs: Vec<(Option<u32>, u32)> = kernel::ENTRIES
        .iter()
        .map(|entry| (entry.level, entry.option))
        .collect();
    let listed = pairs.len();
    pairs.sort_unstable();
    pairs.dedup();
    assert_eq!(pairs.len(), listed, "a pair of numbers with two entries");
    for level in &kernel::LEVELS {
        for name in level.options {
            assert!(
                kernel::resolve(Some(level.name.path), name.path).is_ok(),
                "{} at {}",
                name.path,
                level.name.path
            );
        }
    }
    for name in &kernel::REQUESTS {
        assert!(kernel::resolve(None, name.path).is_ok(), "{}", name.path);
    }
}

#[test]
fn two_names_of_one_pair_are_one_entry() {
    for (first, second) in [
        ("uapi::SO_ATTACH_FILTER", "uapi::SO_GET_FILTER"),
        ("uapi::SO_TIMESTAMPING", "uapi::SO_TIMESTAMPING_OLD"),
    ] {
        let one =
            kernel::resolve(Some("uapi::SOL_SOCKET"), first).expect("the first name resolves");
        let other =
            kernel::resolve(Some("uapi::SOL_SOCKET"), second).expect("the second name resolves");
        assert!(std::ptr::eq(one, other), "{first} and {second}");
    }
}

#[test]
fn a_name_resolves_only_in_its_own_levels_number_space() {
    // TCP_CC_INFO is 26 at SOL_TCP, the number of SO_ATTACH_FILTER at SOL_SOCKET; MCAST_JOIN_GROUP is 42, the
    // number of TCP_AO_REPAIR at SOL_TCP.
    for (level, option) in [
        ("uapi::SOL_SOCKET", "uapi::TCP_CC_INFO"),
        ("uapi::SOL_TCP", "uapi::MCAST_JOIN_GROUP"),
        ("uapi::SOL_TCP", "uapi::SO_ATTACH_FILTER"),
        ("uapi::SOL_IP", "uapi::IPV6_FLOWLABEL_MGR"),
    ] {
        assert!(
            kernel::resolve(Some(level), option).is_err(),
            "{option} at {level}"
        );
    }
    assert!(kernel::resolve(None, "uapi::SO_RCVMARK").is_err());
    assert!(kernel::resolve(Some("uapi::SOL_TCP"), "local::SIOCOUTQ").is_err());
}

// One property per marked reason of the direct crate, each over the code the generator makes for a row.

/// Gives the private function a generated method calls.
fn routed(row: &Row<'_>) -> String {
    body(row)
        .split('(')
        .next()
        .unwrap_or_default()
        .split("::")
        .next()
        .unwrap_or_default()
        .to_owned()
}

/// Checks `set_bytes_at`'s reason: a value lent is what the kernel reads at the pair, and no address it follows.
///
/// The value comes from an exclusive borrow where the kernel writes into it, and the one address the kernel follows is
/// in the program header `set_program` writes.
fn set_reason(row: &Row<'_>, entry: &Entry) -> Result<(), String> {
    let route = routed(row);
    if !route.starts_with(SET_PREFIX) {
        return Ok(());
    }
    let does = entry.set;
    let (_, parameters, _) = signature(row);
    let value = row.value;
    let sound = match (route.as_str(), does) {
        ("set_number", Does::ReadsNumber)
        | ("set_program", Does::FollowsProgram)
        | ("set_group_filter", Does::ReadsGroupFilter) => true,
        ("set_value", Does::ReadsBytes) => value == "bytes" && parameters.contains("&[u8]"),
        ("set_value", Does::ReadsStructure(read)) => {
            value.strip_prefix(STRUCT_FORM) == Some(read) && !parameters.contains("&mut")
        }
        ("set_value", Does::ReadsRun(read)) => value.strip_prefix(SLICE_FORM) == Some(read),
        ("set_value_written", Does::WritesBack(written)) => {
            value.strip_prefix(STRUCT_FORM) == Some(written) && parameters.contains("&mut")
        }
        _ => false,
    };
    if sound {
        Ok(())
    } else {
        Err(format!(
            "{}: routed to {route} with a {value} value where the kernel does {does:?}",
            row.method
        ))
    }
}

/// Checks `get_within`'s reason: a get reaches it only where the kernel writes at most the length told.
///
/// The kernel follows no address in it there, except through `get_keys`, where it writes one answer per key asked.
fn get_within_reason(row: &Row<'_>, entry: &Entry) -> Result<(), String> {
    let route = routed(row);
    let sound = match route.as_str() {
        "get_value" | "get_written" => entry.get == Does::WritesWithin,
        "get_keys" => entry.get == Does::WritesPerKey,
        _ => true,
    };
    if sound {
        Ok(())
    } else {
        Err(format!(
            "{}: routed to {route} where the kernel does {:?}",
            row.method, entry.get
        ))
    }
}

/// `full_info_into`: only the pair whose kernel reads `struct mptcp_full_info` and writes its two runs reaches it.
fn full_info_reason(row: &Row<'_>, entry: &Entry) -> Result<(), String> {
    let routed_here = routed(row) == "get_full_info";
    if routed_here == (!row.sets() && entry.get == Does::WritesThroughFullInfo) {
        Ok(())
    } else {
        Err(format!(
            "{}: get_full_info {routed_here} where the kernel does {:?}",
            row.method, entry.get
        ))
    }
}

/// `ioctl_within`: only a request whose kernel writes one int reaches it, with no level.
fn ioctl_reason(row: &Row<'_>, entry: &Entry) -> Result<(), String> {
    let routed_here = routed(row) == "ioctl_int";
    let request = entry.level.is_none() && row.level == NO_LEVEL;
    if routed_here == (request && entry.get == Does::WritesInt) {
        Ok(())
    } else {
        Err(format!(
            "{}: ioctl_int {routed_here} where the kernel does {:?}",
            row.method, entry.get
        ))
    }
}

/// Every reason's descriptor: the method is an identifier, so its only descriptor is the borrowed `socket`.
fn descriptor_reason(row: &Row<'_>, _: &Entry) -> Result<(), String> {
    if identifier(row.method) {
        Ok(())
    } else {
        Err(format!(
            "{}: not an identifier, so the method's text is not the one the generator writes",
            row.method
        ))
    }
}

/// Every reason's family: the socket reaches the call only through the check of the families its level was read in.
fn family_reason(row: &Row<'_>, _: &Entry) -> Result<(), String> {
    family_check(row, &body(row))
}

/// Checks that `code`, a method's body, passes its socket once, through exactly the family check the kernel table's
/// families of the row's level give, or through none where the level is every family's.
fn family_check(row: &Row<'_>, code: &str) -> Result<(), String> {
    let level = (row.level != NO_LEVEL).then_some(row.level);
    let method = row.method;
    let families =
        kernel::families(level).ok_or_else(|| format!("{method}: its level names no families"))?;
    let passed = code.matches("socket").count();
    let sound = match families {
        Families::Every => passed == 1 && !code.contains(OPTION_FAMILY_CHECK),
        Families::Only(names) => {
            let list: Vec<&str> = names.iter().map(|name| name.path).collect();
            let check = if level.is_none() {
                REQUEST_FAMILY_CHECK
            } else {
                OPTION_FAMILY_CHECK
            };
            passed == 1 && code.contains(&format!("{check}(socket, &[{}])?", list.join(", ")))
        }
    };
    if sound {
        Ok(())
    } else {
        Err(format!(
            "{method}: the socket does not pass the check of its level's families in {code}"
        ))
    }
}

/// A property of a marked reason, over one row and the entry its names resolve to.
type Reason = fn(&Row<'_>, &Entry) -> Result<(), String>;

/// Every marked reason's property, by the function whose `unsafe` it lets stand.
const REASONS: [(&str, Reason); 6] = [
    ("set_bytes_at", set_reason),
    ("get_within", get_within_reason),
    ("full_info_into", full_info_reason),
    ("ioctl_within", ioctl_reason),
    ("every call's descriptor", descriptor_reason),
    ("every call's family", family_reason),
];

#[test]
fn a_body_whose_socket_skips_or_changes_its_family_check_is_seen_by_the_family_property() {
    let table =
        fs::read_to_string(crate_folder().join("options.table")).expect("the table is there");
    let mut checked = 0;
    for row in rows(&table) {
        let Some(guard) = family_guard(&row) else {
            continue;
        };
        let code = body(&row);
        for planted in [
            code.replace(&guard, "socket"),
            code.replace("uapi::AF_INET6", "uapi::AF_UNIX"),
            code.replace("uapi::AF_INET, ", ""),
            format!("{code}; set_number(socket, 0, 0, &0)"),
        ] {
            assert!(
                family_check(&row, &planted).is_err(),
                "the family property misses {planted}"
            );
        }
        checked += 1;
    }
    assert!(checked > 0, "no row of the table has a family check");
}

#[test]
fn every_level_but_the_socket_layers_names_the_internet_families() {
    for level in &kernel::LEVELS {
        let every = matches!(level.families, Families::Every);
        assert_eq!(
            every,
            level.name.path == "uapi::SOL_SOCKET",
            "{}",
            level.name.path
        );
        if let Families::Only(names) = level.families {
            let numbers: Vec<u32> = names.iter().map(|name| name.number).collect();
            assert_eq!(
                numbers,
                [libc::AF_INET as u32, libc::AF_INET6 as u32],
                "{}",
                level.name.path
            );
        }
    }
    assert!(matches!(kernel::REQUEST_FAMILIES, Families::Only(_)));
}

#[test]
fn every_row_of_the_table_meets_every_marked_reason() {
    let table =
        fs::read_to_string(crate_folder().join("options.table")).expect("the table is there");
    for row in rows(&table) {
        let entry = entry(&row).expect("every row resolves");
        for (reason, property) in REASONS {
            if let Err(fault) = property(&row, entry) {
                panic!("{reason}: {fault}");
            }
        }
    }
}

/// A planted row for each marked reason that breaks its property, which the generator must also refuse.
///
/// The entry is resolved apart from the refusal, so the property is measured on the code the row would make.
#[test]
fn a_planted_row_breaking_each_marked_reason_is_seen_by_its_property_and_refused() {
    let planted_rows = [
        // The kernel writes the chosen label into the request: lent shared, the kernel would write behind `&`.
        (
            "set_bytes_at",
            planted(
                "set_planted",
                "uapi::SOL_IPV6",
                "uapi::IPV6_FLOWLABEL_MGR",
                "struct:uapi::in6_flowlabel_req",
                READS,
            ),
        ),
        // SO_GET_FILTER is SO_ATTACH_FILTER: two plain values spelling a program header the kernel follows.
        (
            "set_bytes_at",
            planted(
                "set_planted",
                "uapi::SOL_SOCKET",
                "uapi::SO_GET_FILTER",
                "slice:uapi::tcp_repair_opt",
                READS,
            ),
        ),
        // The kernel reads a run of repair options, given one group request.
        (
            "set_bytes_at",
            planted(
                "set_planted",
                "uapi::SOL_TCP",
                "uapi::TCP_REPAIR_OPTIONS",
                "struct:uapi::group_req",
                READS,
            ),
        ),
        // A room for the key query, whose kernel writes one answer per key asked.
        (
            "get_within",
            planted(
                "get_planted",
                "uapi::SOL_TCP",
                "uapi::TCP_AO_GET_KEYS",
                "room",
                PAST,
            ),
        ),
        // A room for SO_GET_FILTER, whose kernel counts the length told in instructions of eight bytes.
        (
            "get_within",
            planted(
                "get_planted",
                "uapi::SOL_SOCKET",
                "uapi::SO_GET_FILTER",
                "room",
                WITHIN,
            ),
        ),
        // A room for TCP_ZEROCOPY_RECEIVE, whose value holds the address the kernel maps pages at.
        (
            "get_within",
            planted(
                "get_planted",
                "uapi::SOL_TCP",
                "uapi::TCP_ZEROCOPY_RECEIVE",
                "room",
                WITHIN,
            ),
        ),
        // The full information form on MPTCP_INFO, whose kernel follows no address the header would hold.
        (
            "full_info_into",
            planted(
                "get_planted",
                "uapi::SOL_MPTCP",
                "local::MPTCP_INFO",
                "full_info",
                THROUGH,
            ),
        ),
        // MPTCP_FULL_INFO read as a room: the kernel would follow the addresses in the room's bytes.
        (
            "full_info_into",
            planted(
                "get_planted",
                "uapi::SOL_MPTCP",
                "local::MPTCP_FULL_INFO",
                "room",
                WITHIN,
            ),
        ),
        // SIOCGSTAMP writes sixteen bytes into the room of one int.
        (
            "ioctl_within",
            planted(
                "get_planted",
                NO_LEVEL,
                "uapi::SIOCGSTAMP_OLD",
                "ioctl",
                WITHIN,
            ),
        ),
        // A method name that writes a method of its own, taking a raw descriptor.
        (
            "every call's descriptor",
            planted(
                "set_x(&self,fd:i32)->Result<(),HostError>;fn set_y",
                "uapi::SOL_SOCKET",
                "uapi::SO_RCVMARK",
                "bool",
                READS,
            ),
        ),
    ];
    for (name, row) in planted_rows {
        let (_, property) = REASONS
            .iter()
            .find(|(reason, _)| *reason == name)
            .expect("a reason of that name");
        let entry = entry(&row).expect("the planted row resolves, so its property is measured");
        assert!(
            property(&row, entry).is_err(),
            "{name}'s property misses {} {} {}",
            row.option,
            row.value,
            row.kernel
        );
        assert!(
            refusal(&row).is_some(),
            "the generator takes {} {} {}",
            row.option,
            row.value,
            row.kernel
        );
    }
}

// Rows an attacker writes into the table, each of which must be refused.

/// Thirty hostile rows an attacker could write into the table.
///
/// They give the same number under another name, another level, a structure that holds an address, a length the kernel
/// trusts, an option that changes how a later one reads, an ioctl number that aliases, and text that would write code.
const HOSTILE: [(&str, &str, &str, &str, &str); 30] = [
    // 1-4: SO_GET_FILTER is SO_ATTACH_FILTER's number 26.
    (
        "set_so_get_filter",
        "uapi::SOL_SOCKET",
        "uapi::SO_GET_FILTER",
        "slice:uapi::tcp_repair_opt",
        READS,
    ),
    (
        "get_so_get_filter",
        "uapi::SOL_SOCKET",
        "uapi::SO_GET_FILTER",
        "room",
        WITHIN,
    ),
    (
        "get_so_get_filter_count",
        "uapi::SOL_SOCKET",
        "uapi::SO_GET_FILTER",
        "c_int",
        WITHIN,
    ),
    (
        "set_so_attach_filter_bytes",
        "uapi::SOL_SOCKET",
        "uapi::SO_ATTACH_FILTER",
        "bytes",
        READS,
    ),
    // 5-8: another level's name for the same number.
    (
        "set_planted_cross",
        "uapi::SOL_SOCKET",
        "uapi::TCP_CC_INFO",
        "slice:uapi::tcp_repair_opt",
        READS,
    ),
    (
        "set_mcast_at_tcp",
        "uapi::SOL_TCP",
        "uapi::MCAST_JOIN_GROUP",
        "struct:uapi::group_req",
        READS,
    ),
    (
        "set_filter_at_tcp",
        "uapi::SOL_TCP",
        "uapi::SO_ATTACH_FILTER",
        "program",
        READS,
    ),
    (
        "get_ioctl_as_option",
        "uapi::SOL_TCP",
        "local::SIOCOUTQ",
        "c_int",
        WITHIN,
    ),
    // 9-10: the right name at its level, given another structure of the right size.
    (
        "set_ao_repair_as_group",
        "uapi::SOL_TCP",
        "uapi::TCP_AO_REPAIR",
        "struct:uapi::group_req",
        READS,
    ),
    (
        "set_repair_options_as_one",
        "uapi::SOL_TCP",
        "uapi::TCP_REPAIR_OPTIONS",
        "struct:uapi::tcp_repair_opt",
        READS,
    ),
    // 11-14: a structure that holds an address the kernel follows.
    (
        "set_fprog",
        "uapi::SOL_SOCKET",
        "uapi::SO_ATTACH_FILTER",
        "struct:libc::sock_fprog",
        READS,
    ),
    (
        "set_iovecs",
        "uapi::SOL_TCP",
        "uapi::TCP_REPAIR_OPTIONS",
        "slice:libc::iovec",
        READS,
    ),
    (
        "get_zerocopy",
        "uapi::SOL_TCP",
        "uapi::TCP_ZEROCOPY_RECEIVE",
        "room",
        WITHIN,
    ),
    (
        "get_full_as_room",
        "uapi::SOL_MPTCP",
        "local::MPTCP_FULL_INFO",
        "room",
        WITHIN,
    ),
    // 15-18: a length or count inside the value that the kernel trusts.
    (
        "get_keys_room",
        "uapi::SOL_TCP",
        "uapi::TCP_AO_GET_KEYS",
        "room",
        PAST,
    ),
    (
        "get_keys_within",
        "uapi::SOL_TCP",
        "uapi::TCP_AO_GET_KEYS",
        "room",
        WITHIN,
    ),
    (
        "get_info_full",
        "uapi::SOL_MPTCP",
        "local::MPTCP_INFO",
        "full_info",
        THROUGH,
    ),
    (
        "get_syn_keys",
        "uapi::SOL_TCP",
        "uapi::TCP_SAVED_SYN",
        "keys",
        PAST,
    ),
    // 19-21: a value the kernel writes into, lent shared; or a number marked written back.
    (
        "set_flowlabel_shared",
        "uapi::SOL_IPV6",
        "uapi::IPV6_FLOWLABEL_MGR",
        "struct:uapi::in6_flowlabel_req",
        READS,
    ),
    (
        "set_flowlabel_number",
        "uapi::SOL_IPV6",
        "uapi::IPV6_FLOWLABEL_MGR",
        "u64",
        WRITES_BACK,
    ),
    (
        "set_options_written",
        "uapi::SOL_IP",
        "uapi::IP_OPTIONS",
        "bytes",
        WRITES_BACK,
    ),
    // 22-23: an option that changes how a later one reads its value: TCP_ULP and the SOL_TLS level it opens.
    (
        "set_tcp_ulp",
        "uapi::SOL_TCP",
        "uapi::TCP_ULP",
        "bytes",
        READS,
    ),
    (
        "set_tls_tx",
        "uapi::SOL_TLS",
        "uapi::TLS_TX",
        "bytes",
        READS,
    ),
    // 24-26: an ioctl number that aliases, or a request given as an option.
    (
        "get_siocgstamp",
        NO_LEVEL,
        "uapi::SIOCGSTAMP_OLD",
        "ioctl",
        WITHIN,
    ),
    (
        "get_siocoutq_at_level",
        "uapi::SOL_SOCKET",
        "local::SIOCOUTQ",
        "ioctl",
        WITHIN,
    ),
    (
        "get_option_as_request",
        NO_LEVEL,
        "uapi::SO_RCVMARK",
        "ioctl",
        WITHIN,
    ),
    // 27-30: names that resolve nowhere, or text that writes code.
    (
        "set_number_name",
        "uapi::SOL_SOCKET",
        "26",
        "program",
        READS,
    ),
    (
        "set_libc_path",
        "uapi::SOL_SOCKET",
        "libc::SO_ATTACH_FILTER",
        "program",
        READS,
    ),
    (
        "set_x(&self,fd:i32)->Result<(),HostError>;fn_set_y",
        "uapi::SOL_SOCKET",
        "uapi::SO_RCVMARK",
        "bool",
        READS,
    ),
    (
        "get_meminfo_past",
        "uapi::SOL_SOCKET",
        "uapi::SO_MEMINFO",
        "room",
        PAST,
    ),
];

#[test]
fn every_hostile_row_is_refused() {
    let taken: Vec<String> = HOSTILE
        .iter()
        .enumerate()
        .filter_map(|(index, (method, level, option, value, kernel))| {
            let row = planted(method, level, option, value, kernel);
            let reason = refusal(&row);
            println!(
                "HOSTILE {} {method} {option} {value} {kernel}: {reason:?}",
                index + 1
            );
            reason.is_none().then(|| format!("{} {method}", index + 1))
        })
        .collect();
    assert!(taken.is_empty(), "hostile rows taken: {taken:?}");
}
