//! Checks, from the kernel's uapi headers as linux-raw-sys's bindgen renders them, that no value the primitive passes
//! the kernel can hold an address it follows: every structure on the list is read field by field, and every value
//! that is not a 4-byte integer or such a structure must be one this file names as reviewed.

use std::collections::BTreeMap;
use std::path::PathBuf;

use wcontainers_socket_options::{PlainOption, Value};

/// Fields of 8 bytes that the kernel never follows as an address, each with its reason.
const NOT_ADDRESSES: &[(&str, &str, &str)] = &[
    (
        "__kernel_sockaddr_storage__bindgen_ty_1",
        "__align",
        "a union member that only aligns the address bytes: sockaddr_storage is read as a family and address",
    ),
    ("tcp_ao_info_opt", "pkt_good", "a packet counter"),
    ("tcp_ao_info_opt", "pkt_bad", "a packet counter"),
    ("tcp_ao_info_opt", "pkt_key_not_found", "a packet counter"),
    ("tcp_ao_info_opt", "pkt_ao_required", "a packet counter"),
    ("tcp_ao_info_opt", "pkt_dropped_icmp", "a packet counter"),
];

/// The options set with bytes the kernel parses, each read in the kernel's source for an address: none holds one.
const REVIEWED_BYTES: &[PlainOption] = &[
    // IP option bytes (ip_options_get, net/ipv4/ip_options.c).
    PlainOption::IpOptions,
    // IPv6 extension headers (ipv6_renew_options, net/ipv6/exthdrs.c).
    PlainOption::Ipv6Hopopts,
    PlainOption::Ipv6Rthdrdstopts,
    PlainOption::Ipv6Rthdr,
    PlainOption::Ipv6Dstopts,
    // One or two 16-byte keys (do_tcp_setsockopt, net/ipv4/tcp.c).
    PlainOption::TcpFastopenKey,
    // An array of `tcp_repair_opt`, two `__u32` each (tcp_repair_options_est, net/ipv4/tcp.c).
    PlainOption::TcpRepairOptions,
];

/// The options whose 64-bit value the kernel reads as a number.
const REVIEWED_NUMBERS: &[PlainOption] = &[
    // A rate in bytes a second (sk_setsockopt, net/core/sock.c).
    PlainOption::SoMaxPacingRate,
];

/// Field types of eight bytes or more, as bindgen names them: wide enough for an address.
const WIDE: &[&str] = &[
    "u64",
    "i64",
    "usize",
    "isize",
    "crate::ctypes::c_ulong",
    "crate::ctypes::c_long",
    "crate::ctypes::c_ulonglong",
    "crate::ctypes::c_longlong",
    "c_ulong",
    "c_long",
    "c_ulonglong",
    "c_longlong",
];

/// linux-raw-sys's `net` module for this architecture, found through `cargo metadata`.
fn bindings() -> String {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let metadata = std::process::Command::new(cargo)
        .args(["metadata", "--format-version", "1", "--offline"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo metadata runs");
    let text = String::from_utf8(metadata.stdout).expect("cargo metadata gives UTF-8");
    let manifest = text
        .split("\"manifest_path\":\"")
        .skip(1)
        .filter_map(|rest| rest.split('"').next())
        .find(|path| path.contains("/linux-raw-sys-0.12."))
        .expect("linux-raw-sys 0.12 is in the graph");
    let file = PathBuf::from(manifest)
        .with_file_name("src")
        .join(std::env::consts::ARCH)
        .join("net.rs");
    std::fs::read_to_string(&file).unwrap_or_else(|error| panic!("{}: {error}", file.display()))
}

/// Each struct and union of the bindings, by name, with its fields' names and types.
type Aggregates = BTreeMap<String, Vec<(String, String)>>;
/// Each type alias of the bindings, by name, with the type it names.
type Aliases = BTreeMap<String, String>;

/// Every struct and union of the bindings, and every type alias.
fn items(source: &str) -> (Aggregates, Aliases) {
    let (mut aggregates, mut aliases) = (BTreeMap::new(), BTreeMap::new());
    let mut open: Option<(String, Vec<(String, String)>)> = None;
    for line in source.lines().map(str::trim) {
        if let Some((name, fields)) = &mut open {
            if line == "}" {
                aggregates.insert(name.clone(), std::mem::take(fields));
                open = None;
            } else if let Some(field) = line.strip_prefix("pub ") {
                let (field, kind) = field.split_once(':').expect("a field has a type");
                fields.push((field.trim().into(), kind.trim().trim_end_matches(',').into()));
            }
            continue;
        }
        for keyword in ["pub struct ", "pub union "] {
            if let Some(rest) = line.strip_prefix(keyword)
                && let Some(name) = rest.strip_suffix(" {")
            {
                open = Some((name.split('<').next().unwrap_or(name).into(), Vec::new()));
            }
        }
        if let Some(rest) = line.strip_prefix("pub type ")
            && let Some((name, kind)) = rest.split_once(" = ")
        {
            aliases.insert(name.trim().into(), kind.trim_end_matches(';').trim().into());
        }
    }
    (aggregates, aliases)
}

/// The fields of the aggregate, and of every aggregate inside it, that are wide enough to hold an address.
fn wide_fields(
    name: &str,
    aggregates: &Aggregates,
    aliases: &Aliases,
    found: &mut Vec<(String, String, String)>,
) {
    let fields = aggregates.get(name).unwrap_or_else(|| panic!("{name} is not in the bindings"));
    for (field, kind) in fields {
        let mut kind = kind.as_str();
        loop {
            kind = kind.trim();
            if let Some(inner) = kind.strip_prefix('[') {
                kind = inner.rsplit_once(';').map_or(inner, |(element, _)| element);
            } else if let Some(inner) = kind.strip_prefix("__IncompleteArrayField<") {
                kind = inner.trim_end_matches('>');
            } else if let Some(resolved) = aliases.get(kind) {
                kind = resolved;
            } else {
                break;
            }
        }
        if kind.starts_with('*') || WIDE.contains(&kind) {
            found.push((name.into(), field.clone(), kind.into()));
        } else if kind.starts_with("__BindgenBitfieldUnit") {
        } else if aggregates.contains_key(kind) {
            wide_fields(kind, aggregates, aliases, found);
        }
    }
}

#[test]
fn no_listed_structure_holds_an_address() {
    let source = bindings();
    let (aggregates, aliases) = items(&source);
    let mut found = Vec::new();
    let (mut structures, mut bytes, mut numbers, mut filled, mut ints) = (0, Vec::new(), Vec::new(), 0, 0);
    for &option in PlainOption::ALL {
        match option.value() {
            Value::Int => ints += 1,
            Value::Filled => filled += 1,
            Value::Structure { name, .. } => {
                structures += 1;
                wide_fields(name, &aggregates, &aliases, &mut found);
            }
            Value::Bytes => bytes.push(option),
            Value::Number64 => numbers.push(option),
        }
    }
    found.sort();
    found.dedup();
    let unexplained: Vec<_> = found
        .iter()
        .filter(|(aggregate, field, _)| {
            !NOT_ADDRESSES.iter().any(|(named, named_field, _)| named == aggregate && named_field == field)
        })
        .collect();
    eprintln!(
        "{} options: {ints} 4-byte integers, {filled} only read, {structures} structures, {} byte strings and {} 64-bit \
         numbers reviewed; wide fields found {found:?}",
        PlainOption::ALL.len(),
        bytes.len(),
        numbers.len(),
    );
    assert!(unexplained.is_empty(), "fields wide enough for an address: {unexplained:?}");
    assert_eq!(bytes, REVIEWED_BYTES, "a byte-string option not reviewed for addresses");
    assert_eq!(numbers, REVIEWED_NUMBERS, "a 64-bit option not reviewed as a number");
}
