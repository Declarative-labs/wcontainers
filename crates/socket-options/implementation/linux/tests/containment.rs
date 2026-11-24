//! The third independent check's tests of the containment, kept as this primitive's own suite, run as root on x86-64.
//!
//! The kernel's copy helpers cannot be probed, so a tracefs instance records every page fault the calling thread takes
//! in kernel mode (`exceptions:page_fault_kernel`, which x86-64 alone has) and puts each faulting address in a class:
//! the 4 GiB of scratch file before the data, the data, the guard past it, the thread's stack, the primitive's own
//! buffers as its `pread` and `pwrite` fill and empty them, or OTHER. Every access to the guard faults, and so does any
//! access to a page of ours that is not resident (the untouched part of the scratch file, an untouched decoy, an
//! unmapped address), so an OTHER line is the kernel touching memory the primitive did not lend. The descriptor swap
//! swaps fd 0, which no other test of this file uses.
#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::mem::{offset_of, size_of};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::os::unix::fs::OpenOptionsExt;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use linux_raw_sys::net as uapi;
use memmap2::MmapOptions;
use rustix::io::Errno;
use rustix::net::{AddressFamily, Protocol, SocketType, ipproto};
use wcontainers_socket_options::{
    AUTHENTICATION_KEY_QUERY, MulticastLevel, PlainOption, QueuedCount, SocketOptions, SourceFilterOptions,
    StorageBytes, Value,
};
use wcontainers_socket_options_linux::LinuxSocketOptions as L;

const MARK: u8 = 0xA5;
const PAGE: usize = 4096;
/// The scratch file's bytes before the data, and the unbacked bytes after it, as the primitive maps them.
const BEFORE: u64 = 1 << 32;
const GUARD: u64 = 1 << 32;
const COUNTS: [QueuedCount; 3] = [QueuedCount::SendQueue, QueuedCount::Unsent, QueuedCount::AtUrgentMark];
/// Calls made while another thread swaps the descriptor, for each kind of descriptor.
const RACED_CALLS: usize = 20_000;
/// An address below 4 GiB that nothing maps, so that a 32-bit value can name it and a kernel access to it faults.
const LOW_UNMAPPED: u64 = 0x3000_0040;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Mapping {
    start: u64,
    end: u64,
    name: String,
}

fn maps() -> Vec<Mapping> {
    let text = std::fs::read_to_string("/proc/self/maps").expect("the maps are readable");
    text.lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let (start, end) = parts.next()?.split_once('-')?;
            Some(Mapping {
                start: u64::from_str_radix(start, 16).ok()?,
                end: u64::from_str_radix(end, 16).ok()?,
                name: parts.nth(4).unwrap_or_default().to_string(),
            })
        })
        .collect()
}

fn mapping_of(address: u64) -> Option<Mapping> {
    maps().into_iter().find(|mapping| mapping.start <= address && address < mapping.end)
}

/// This thread's scratch, made by a read of `file` bytes on a new thread's first call.
fn fresh_scratch(file: usize) -> Mapping {
    let scratches = || maps().into_iter().filter(|m| m.name.contains("socket-option")).collect::<Vec<_>>();
    let before = scratches();
    let socket = sock(AddressFamily::INET, SocketType::STREAM, Some(ipproto::TCP)).expect("a TCP socket");
    let _ = L.get_option(socket.as_fd(), PlainOption::TcpFastopenKey, &mut vec![u8::MIN; file]);
    let size = BEFORE + file.next_power_of_two().max(PAGE) as u64 + GUARD;
    scratches()
        .into_iter()
        .find(|m| !before.contains(m) && m.end - m.start == size)
        .unwrap_or_else(|| panic!("no fresh scratch for {file} bytes"))
}

/// A buffer of `length` bytes whose pages are all resident, so that the kernel's copies to and from it never fault.
fn touched(length: usize, byte: u8) -> Vec<u8> {
    let mut bytes = vec![1u8; length];
    bytes.fill(byte);
    bytes
}

fn sock(family: AddressFamily, kind: SocketType, protocol: Option<Protocol>) -> Option<OwnedFd> {
    rustix::net::socket(family, kind, protocol).ok()
}

/// A socket made from raw numbers, for families rustix does not name.
fn numbered(family: u16, kind: u32, protocol: u32) -> Option<OwnedFd> {
    let protocol = rustix::net::RawProtocol::new(protocol).map(Protocol::from_raw);
    sock(AddressFamily::from_raw(family), SocketType::from_raw(kind), protocol)
}

/// A loopback connection: the accepted side, the connecting side, the listener.
fn pair(family: AddressFamily, protocol: Protocol) -> Option<(OwnedFd, OwnedFd, OwnedFd)> {
    let loopback =
        if family == AddressFamily::INET6 { IpAddr::V6(Ipv6Addr::LOCALHOST) } else { IpAddr::V4(Ipv4Addr::LOCALHOST) };
    let listener = sock(family, SocketType::STREAM, Some(protocol))?;
    rustix::net::bind(&listener, &SocketAddr::new(loopback, 0)).ok()?;
    rustix::net::listen(&listener, 8).ok()?;
    let at = rustix::net::getsockname(&listener).ok()?;
    let connecting = sock(family, SocketType::STREAM, Some(protocol))?;
    rustix::net::connect(&connecting, &at).ok()?;
    let accepted = rustix::net::accept(&listener).ok()?;
    Some((accepted, connecting, listener))
}

fn storage(address: IpAddr) -> StorageBytes {
    let mut storage = [0u8; size_of::<StorageBytes>()];
    match address {
        IpAddr::V4(v4) => {
            storage[..2].copy_from_slice(&(uapi::AF_INET as u16).to_ne_bytes());
            storage[4..8].copy_from_slice(&v4.octets());
        }
        IpAddr::V6(v6) => {
            storage[..2].copy_from_slice(&(uapi::AF_INET6 as u16).to_ne_bytes());
            storage[8..24].copy_from_slice(&v6.octets());
        }
    }
    storage
}

fn put(bytes: &mut [u8], at: usize, value: &[u8]) {
    bytes[at..at + value.len()].copy_from_slice(value);
}

/// A value of the option's kind that the kernel would take on a fitting socket, before any address is planted.
fn valid_value(option: PlainOption, v6: bool, id: u8) -> Vec<u8> {
    let loopback = if v6 { IpAddr::V6(Ipv6Addr::LOCALHOST) } else { IpAddr::V4(Ipv4Addr::LOCALHOST) };
    let group = if v6 {
        IpAddr::V6(Ipv6Addr::new(0xff3e, 0, 0, 0, 0, 0, 0x4321, 1))
    } else {
        IpAddr::V4(Ipv4Addr::new(232, 4, 3, id))
    };
    match option.value() {
        Value::Int => 1i32.to_ne_bytes().to_vec(),
        Value::Number64 => 1_000_000u64.to_ne_bytes().to_vec(),
        Value::Filled => Vec::new(),
        Value::Bytes => match option {
            PlainOption::TcpFastopenKey => vec![id; 16],
            PlainOption::TcpRepairOptions => vec![2, 0, 0, 0, 0xb4, 0x05, 0, 0],
            PlainOption::IpOptions => vec![1; 8],
            _ => vec![0, 0, 1, 4, 0, 0, 0, 0],
        },
        Value::Structure { size, name } => {
            let mut value = vec![0u8; size];
            match name {
                "group_req" => {
                    put(&mut value, 0, &1u32.to_ne_bytes());
                    put(&mut value, offset_of!(uapi::group_req, gr_group), &storage(group));
                }
                "tcp_ao_add" => {
                    put(&mut value, 0, &storage(loopback));
                    put(&mut value, offset_of!(uapi::tcp_ao_add, alg_name), b"hmac(sha256)");
                    value[offset_of!(uapi::tcp_ao_add, prefix)] = if v6 { 128 } else { 32 };
                    value[offset_of!(uapi::tcp_ao_add, sndid)] = id;
                    value[offset_of!(uapi::tcp_ao_add, rcvid)] = id;
                    value[offset_of!(uapi::tcp_ao_add, keylen)] = 16;
                    put(&mut value, offset_of!(uapi::tcp_ao_add, key), &[id; 16]);
                }
                "tcp_md5sig" => {
                    put(&mut value, 0, &storage(loopback));
                    put(&mut value, offset_of!(uapi::tcp_md5sig, tcpm_keylen), &16u16.to_ne_bytes());
                    put(&mut value, offset_of!(uapi::tcp_md5sig, tcpm_key), &[id; 16]);
                }
                "ipv6_mreq" => {
                    put(&mut value, 0, &Ipv6Addr::LOCALHOST.octets());
                    put(&mut value, 16, &1i32.to_ne_bytes());
                }
                _ => {}
            }
            value
        }
    }
}

/// What the kernel faulted on during a traced run, by class, and each OTHER fault described.
#[derive(Default, Debug)]
struct Tally {
    classes: BTreeMap<&'static str, u64>,
    other: Vec<String>,
}

impl Tally {
    /// Faults that are not the data, the guard or the stack: the kernel reached memory the primitive did not lend, or
    /// reached back into the scratch before the data, which no handler is known to do.
    fn outside(&self) -> Vec<String> {
        let mut outside = self.other.clone();
        if let Some(count) = self.classes.get("before") {
            outside.push(format!("{count} faults in the scratch before the data"));
        }
        outside
    }
}

/// The places a traced thread's kernel faults are put in.
struct Places {
    scratch: Mapping,
    stack: Mapping,
}

impl Places {
    fn class(&self, address: u64) -> &'static str {
        let data = self.scratch.start + BEFORE;
        let guard = self.scratch.end - GUARD;
        if (self.scratch.start..data).contains(&address) {
            "before"
        } else if (data..guard).contains(&address) {
            "data"
        } else if (guard..self.scratch.end).contains(&address) {
            "guard"
        } else if (self.stack.start..self.stack.end).contains(&address) {
            "stack"
        } else {
            "OTHER"
        }
    }
}

const TRACING: &str = "/sys/kernel/tracing";

fn tracefs_write(path: &str, text: &str) {
    let mut file = std::fs::OpenOptions::new().write(true).truncate(true).open(path).expect("tracefs is writable");
    file.write_all(text.as_bytes()).unwrap_or_else(|error| panic!("{path}: {error}"));
}

/// A tracefs instance recording the kernel-mode page faults of one thread.
struct Tracer {
    instance: String,
}

impl Tracer {
    fn new(thread: i32) -> Tracer {
        let instance = format!("{TRACING}/instances/socket-options-{thread}");
        let _ = std::fs::remove_dir(&instance);
        std::fs::create_dir(&instance).expect("a tracefs instance: run as root");
        tracefs_write(&format!("{instance}/buffer_size_kb"), "1024");
        let event = format!("{instance}/events/exceptions/page_fault_kernel");
        tracefs_write(&format!("{event}/filter"), &format!("common_pid == {thread}"));
        tracefs_write(&format!("{event}/enable"), "1");
        tracefs_write(&format!("{instance}/tracing_on"), "0");
        Tracer { instance }
    }

    /// Runs `body` with the instance on, draining its pipe on another thread, and puts every fault in a class.
    fn run(&self, places: &Places, body: impl FnOnce()) -> Tally {
        tracefs_write(&format!("{}/trace", self.instance), "");
        let stop = AtomicBool::new(false);
        let tally = Mutex::new(Tally::default());
        std::thread::scope(|scope| {
            scope.spawn(|| {
                let mut pipe = std::fs::OpenOptions::new()
                    .read(true)
                    .custom_flags(libc::O_NONBLOCK)
                    .open(format!("{}/trace_pipe", self.instance))
                    .expect("the trace pipe");
                let (mut carry, mut buffer, mut idle) = (String::new(), vec![0u8; 1 << 20], 0);
                loop {
                    match pipe.read(&mut buffer) {
                        Ok(read) if read > 0 => {
                            idle = 0;
                            carry.push_str(&String::from_utf8_lossy(&buffer[..read]));
                            let mut tally = tally.lock().expect("one drainer");
                            while let Some(newline) = carry.find('\n') {
                                let line: String = carry.drain(..=newline).collect();
                                classify(&line, places, &mut tally);
                            }
                        }
                        _ if stop.load(Ordering::SeqCst) && idle > 20 => break,
                        _ => {
                            idle += usize::from(stop.load(Ordering::SeqCst));
                            std::thread::sleep(std::time::Duration::from_millis(5));
                        }
                    }
                }
            });
            tracefs_write(&format!("{}/tracing_on", self.instance), "1");
            body();
            tracefs_write(&format!("{}/tracing_on", self.instance), "0");
            stop.store(true, Ordering::SeqCst);
        });
        let mut tally = tally.into_inner().expect("one drainer");
        let overrun: u64 = std::fs::read_dir(format!("{}/per_cpu", self.instance))
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|cpu| std::fs::read_to_string(cpu.path().join("stats")).ok())
            .flat_map(|stats| {
                stats.lines().filter_map(|line| line.strip_prefix("overrun: ")?.trim().parse::<u64>().ok()).collect::<Vec<_>>()
            })
            .sum();
        if overrun > 0 {
            tally.other.push(format!("{overrun} trace events lost"));
        }
        tally
    }
}

impl Drop for Tracer {
    fn drop(&mut self) {
        let _ = std::fs::write(format!("{}/tracing_on", self.instance), "0");
        let _ = std::fs::write(format!("{}/events/exceptions/page_fault_kernel/enable", self.instance), "0");
        let _ = std::fs::remove_dir(&self.instance);
    }
}

fn classify(line: &str, places: &Places, tally: &mut Tally) {
    if !line.contains("page_fault_kernel:") {
        return;
    }
    let field = |key: &str| {
        line.split_whitespace()
            .find_map(|word| word.strip_prefix(key))
            .and_then(|value| u64::from_str_radix(value.trim_start_matches("0x"), 16).ok())
    };
    let Some(address) = field("address=") else { return };
    // The tracepoint prints the instruction as a symbol.
    let ip = line.split_whitespace().find_map(|word| word.strip_prefix("ip=")).unwrap_or("?");
    let class = match places.class(address) {
        // The primitive's own `pread` and `pwrite` into its buffers, which no socket handler's copy goes through.
        "OTHER" if ip.starts_with("_copy_to_iter") || ip.starts_with("_copy_from_iter") => "buffer",
        class => class,
    };
    *tally.classes.entry(class).or_default() += 1;
    if class == "OTHER" && tally.other.len() < 30 {
        let near = mapping_of(address).map(|m| format!("{:x}-{:x} {}", m.start, m.end, m.name));
        tally.other.push(format!("fault at {address:#x} from {ip} in {near:?}"));
    }
}

/// Runs `body` on a new thread whose scratch holds `file` bytes, with that thread's kernel faults traced.
fn traced<R: Send>(file: usize, body: impl FnOnce() -> R + Send) -> (R, Tally) {
    std::thread::scope(|scope| {
        scope
            .spawn(|| {
                let tracer = Tracer::new(rustix::thread::gettid().as_raw_nonzero().get());
                let scratch = fresh_scratch(file);
                let local = u8::MIN;
                let stack = mapping_of(std::ptr::addr_of!(local).addr() as u64).expect("the stack is mapped");
                let places = Places { scratch, stack };
                let mut result = None;
                let tally = tracer.run(&places, || result = Some(body()));
                (result.expect("the body ran"), tally)
            })
            .join()
            .expect("the traced thread ends")
    })
}

/// Every method on the socket, each region beginning at the data's first byte of a `file`-byte scratch.
fn every_method_at_full_length(name: &str, socket: BorrowedFd<'_>, file: usize, results: &mut Vec<String>) {
    for &option in PlainOption::ALL {
        let Some(length) = option.value().exact_length() else {
            for background in [u8::MIN, u8::MAX] {
                let mut into = touched(file, background);
                results.push(format!("{name} get {option:?} {file}: {:?}", L.get_option(socket, option, &mut into)));
                if option.value() == Value::Bytes {
                    results.push(format!("{name} set {option:?}: {:?}", L.set_option(socket, option, &into)));
                }
            }
            continue;
        };
        results.push(format!("{name} get {option:?}: {:?}", L.get_option(socket, option, &mut touched(length, 0))));
        if option.value() != Value::Filled {
            results.push(format!("{name} set {option:?}: {:?}", L.set_option(socket, option, &touched(length, 0xff))));
        }
    }
    for count in COUNTS {
        results.push(format!("{name} {count:?}: {:?}", L.count_queued(socket, count)));
    }
    let mut query = [0xffu8; AUTHENTICATION_KEY_QUERY];
    results.push(format!("{name} key: {:?}", L.get_authentication_key(socket, &mut query)));
}

/// Third check (b): every method with each region at the data's first byte of scratch files from a page to a MiB, so
/// that a reach back lands in the scratch before the data and a reach forward in the guard; then the forced lengths.
#[test]
fn the_guard_holds_at_every_length_and_before_the_data() {
    let tcp4 = pair(AddressFamily::INET, ipproto::TCP).expect("a TCP connection");
    let tcp6 = pair(AddressFamily::INET6, ipproto::TCP).expect("a TCP connection");
    let _ = rustix::net::send(&tcp4.1, b"hello", rustix::net::SendFlags::empty());
    let raw6 = sock(AddressFamily::INET6, SocketType::RAW, Some(ipproto::ICMPV6)).expect("a raw socket");
    let mptcp = pair(AddressFamily::INET, ipproto::MPTCP);
    let mut failures = Vec::new();
    for file in [PAGE, 2 * PAGE, 1 << 16, 1 << 20] {
        let (calls, tally) = traced(file, || {
            let mut results = Vec::new();
            for (name, socket) in [("tcp4", tcp4.0.as_fd()), ("tcp6", tcp6.0.as_fd()), ("raw6", raw6.as_fd())] {
                every_method_at_full_length(name, socket, file, &mut results);
            }
            if let Some((_, connecting, _)) = &mptcp {
                // One-byte elements filling the data exactly: the subflow array begins at the data's first byte.
                let count = (file - 40) / 2;
                let (mut subflows, mut information) = (vec![[0u8; 1]; count], vec![[0u8; 1]; count]);
                let answer =
                    L.get_multipath_full_information(connecting.as_fd(), &mut [0u8; 0], &mut subflows, &mut information);
                results.push(format!("mptcp full {count}: {answer:?}"));
            }
            results.len()
        });
        eprintln!("scratch of {file} bytes: {calls} calls; kernel faults by class {:?}", tally.classes);
        failures.extend(tally.outside().into_iter().map(|line| format!("{file}: {line}")));
    }
    for length in [0usize, 1, 3, 7, 4095, 4097, 0x7fff_f000, 0x7fff_f001, 1 << 32] {
        let start = std::time::Instant::now();
        let ((got, set, information), tally) = traced(length, || {
            let mut into = touched(length, u8::MIN);
            let got = L.get_option(tcp4.0.as_fd(), PlainOption::IpOptions, &mut into);
            let set = L.set_option(tcp4.0.as_fd(), PlainOption::IpOptions, &into);
            let information = L.get_option(tcp4.0.as_fd(), PlainOption::TcpInfo, &mut into);
            (got, set, information)
        });
        eprintln!(
            "length {length:#x}: IP_OPTIONS get {got:?}, set {set:?}, TCP_INFO get {information:?}; faults {:?} ({:?})",
            tally.classes,
            start.elapsed()
        );
        if got.is_ok_and(|written| written > length) || information.is_ok_and(|written| written > length) {
            failures.push(format!("{length:#x}: more written than asked"));
        }
        failures.extend(tally.outside().into_iter().map(|line| format!("{length:#x}: {line}")));
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

/// The bytes of this process at the address, read through `/proc/self/mem`.
fn memory(address: u64, length: usize) -> Vec<u8> {
    use std::os::unix::fs::FileExt;
    let mut bytes = vec![0u8; length];
    let memory = std::fs::File::open("/proc/self/mem").expect("the process's memory is readable");
    memory.read_exact_at(&mut bytes, address).expect("the address is mapped");
    bytes
}

/// A region holds zeros when the kernel is given it, whatever an earlier call left there.
#[test]
fn a_region_is_lent_as_zeros() {
    std::thread::spawn(|| {
        let scratch = fresh_scratch(PAGE);
        let region = scratch.start + BEFORE + PAGE as u64 - 40;
        let socket = sock(AddressFamily::INET, SocketType::STREAM, Some(ipproto::TCP)).expect("a TCP socket");
        let refused = L.set_option(socket.as_fd(), PlainOption::IpOptions, &[0xff; 40]);
        let left = memory(region, 40);
        let read = L.get_option(socket.as_fd(), PlainOption::IpOptions, &mut [0xff; 40]);
        let lent = memory(region, 40);
        eprintln!("set {refused:?} left {left:?}; get {read:?} saw {lent:?}");
        assert_eq!((left, read, lent), (vec![0xff; 40], Ok(0), vec![0; 40]));
    })
    .join()
    .expect("the test thread ends");
}

/// The addresses a planted value names: a page of ours filled with a mark, and an address nothing maps.
struct Targets {
    decoy: memmap2::MmapMut,
    unmapped: u64,
}

impl Targets {
    fn new() -> Targets {
        let mut decoy = MmapOptions::new().len(4 * PAGE).map_anon().expect("a decoy");
        decoy.fill(MARK);
        let unmapped = MmapOptions::new().len(4 * PAGE).map_anon().expect("pages to unmap").as_ptr().addr() as u64;
        Targets { decoy, unmapped: unmapped + 64 }
    }

    fn decoy(&self) -> u64 {
        self.decoy.as_ptr().addr() as u64 + 64
    }

    fn written(&mut self) -> usize {
        let written = self.decoy.iter().filter(|&&byte| byte != MARK).count();
        self.decoy.fill(MARK);
        written
    }
}

/// Runs `call` with each target planted; a follow is an answer of `EFAULT` for the unmapped address but not for the
/// decoy, or a write into the decoy. A read through the unmapped address also shows as a fault outside.
fn follows(targets: &mut Targets, call: &mut dyn FnMut(u64) -> Result<(), Errno>) -> Option<String> {
    let unmapped = call(targets.unmapped);
    let decoy = call(targets.decoy());
    let written = targets.written();
    (written > 0 || (unmapped == Err(Errno::FAULT) && decoy != Err(Errno::FAULT)))
        .then(|| format!("unmapped {unmapped:?}, decoy {decoy:?}, {written} decoy bytes written"))
}

/// The bases an address is planted in: zeros, ones, a leading count of 1, every word 1, and a value the kernel takes.
fn bases(option: PlainOption, length: usize, v6: bool, id: u8) -> Vec<Vec<u8>> {
    let mut leading = vec![0u8; length];
    if let Some(first) = leading.first_mut() {
        *first = 1;
    }
    let words = 1u32.to_ne_bytes().iter().copied().cycle().take(length).collect();
    let mut valid = valid_value(option, v6, id);
    valid.resize(length, 0);
    vec![vec![0u8; length], vec![0xffu8; length], leading, words, valid]
}

/// Third check (c), strengthened: an 8-byte address at every 4-byte offset of every settable entry's value, of the
/// key query and of the source filter, and a 32-bit address of an unmapped page below 4 GiB at every 4-byte offset,
/// each in five bases, on eleven kinds of socket.
#[test]
fn no_set_value_carries_an_address_the_kernel_follows() {
    assert!(mapping_of(LOW_UNMAPPED).is_none(), "the low address is mapped");
    let mut sockets: Vec<(&str, OwnedFd, bool)> = Vec::new();
    let mut add = |name, descriptor: Option<OwnedFd>, v6| match descriptor {
        Some(descriptor) => sockets.push((name, descriptor, v6)),
        None => eprintln!("{name}: cannot make"),
    };
    add("tcp4", sock(AddressFamily::INET, SocketType::STREAM, Some(ipproto::TCP)), false);
    add("tcp6", sock(AddressFamily::INET6, SocketType::STREAM, Some(ipproto::TCP)), true);
    let connected = pair(AddressFamily::INET, ipproto::TCP).expect("a TCP connection");
    add("tcp4-connected", rustix::io::dup(&connected.0).ok(), false);
    add("udp4", sock(AddressFamily::INET, SocketType::DGRAM, Some(ipproto::UDP)), false);
    add("udp6", sock(AddressFamily::INET6, SocketType::DGRAM, Some(ipproto::UDP)), true);
    let reuse = sock(AddressFamily::INET, SocketType::DGRAM, Some(ipproto::UDP)).expect("a UDP socket");
    rustix::net::sockopt::set_socket_reuseport(&reuse, true).expect("SO_REUSEPORT");
    rustix::net::bind(&reuse, &SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0)).expect("bound");
    add("udp4-reuseport-bound", Some(reuse), false);
    add("udplite6", sock(AddressFamily::INET6, SocketType::DGRAM, Some(ipproto::UDPLITE)), true);
    add("raw-icmp4", sock(AddressFamily::INET, SocketType::RAW, Some(ipproto::ICMP)), false);
    add("raw-icmp6", sock(AddressFamily::INET6, SocketType::RAW, Some(ipproto::ICMPV6)), true);
    add("raw-tcp4", sock(AddressFamily::INET, SocketType::RAW, Some(ipproto::TCP)), false);
    add("mptcp4", sock(AddressFamily::INET, SocketType::STREAM, Some(ipproto::MPTCP)), false);
    let ((calls, found), tally) = traced(PAGE, || {
        let mut targets = Targets::new();
        let (mut calls, mut found, mut id) = (0u64, Vec::new(), 1u8);
        for (name, socket, v6) in &sockets {
            let socket = socket.as_fd();
            for &option in PlainOption::ALL.iter().filter(|option| option.value() != Value::Filled) {
                let lengths = option.value().exact_length().map_or(vec![8, 16, 24, 32, 40, 64], |length| vec![length]);
                for length in lengths {
                    id = id.wrapping_add(1).max(1);
                    for base in bases(option, length, *v6, id) {
                        for at in (0..length.saturating_sub(7)).step_by(4) {
                            let mut call = |address: u64| {
                                calls += 1;
                                let mut value = base.clone();
                                put(&mut value, at, &address.to_ne_bytes());
                                L.set_option(socket, option, &value)
                            };
                            if let Some(line) = follows(&mut targets, &mut call) {
                                found.push(format!("{name} set {option:?} len {length} address at {at}: {line}"));
                            }
                        }
                        for at in (0..length.saturating_sub(3)).step_by(4) {
                            calls += 1;
                            let mut value = base.clone();
                            put(&mut value, at, &(LOW_UNMAPPED as u32).to_ne_bytes());
                            let _ = L.set_option(socket, option, &value);
                        }
                    }
                }
            }
            for at in (0..AUTHENTICATION_KEY_QUERY - 7).step_by(4) {
                let mut call = |address: u64| {
                    calls += 1;
                    let mut query = [0u8; AUTHENTICATION_KEY_QUERY];
                    put(&mut query, at, &address.to_ne_bytes());
                    L.get_authentication_key(socket, &mut query)
                };
                if let Some(line) = follows(&mut targets, &mut call) {
                    found.push(format!("{name} key query address at {at}: {line}"));
                }
            }
            let group = storage(if *v6 {
                IpAddr::V6(Ipv6Addr::new(0xff3e, 0, 0, 0, 0, 0, 0x4321, 9))
            } else {
                IpAddr::V4(Ipv4Addr::new(232, 9, 9, 9))
            });
            let level = if *v6 { MulticastLevel::Ipv6 } else { MulticastLevel::Ipv4 };
            for at in (0..size_of::<StorageBytes>() - 7).step_by(4) {
                let mut call = |address: u64| {
                    calls += 1;
                    let (mut group, mut source) = (group, group);
                    if at >= 8 {
                        put(&mut group, at, &address.to_ne_bytes());
                    }
                    put(&mut source, at, &address.to_ne_bytes());
                    let sources = [source, source];
                    let filter =
                        SourceFilterOptions { level, interface: 1, group, mode: uapi::MCAST_INCLUDE, sources: &sources };
                    L.set_source_filter(socket, &filter)
                };
                if let Some(line) = follows(&mut targets, &mut call) {
                    found.push(format!("{name} source filter address at {at}: {line}"));
                }
            }
        }
        (calls, found)
    });
    let outside = tally.outside();
    eprintln!(
        "addresses planted: {calls} calls on {} sockets, {} followed or written, {} faults outside; faults by class {:?}",
        sockets.len(),
        found.len(),
        outside.len(),
        tally.classes
    );
    for line in found.iter().chain(&outside).take(20) {
        eprintln!("  {line}");
    }
    assert!(found.is_empty() && outside.is_empty());
}

fn device(path: &str) -> Option<OwnedFd> {
    let open = |write| {
        std::fs::OpenOptions::new().read(true).write(write).custom_flags(libc::O_NONBLOCK).open(path)
    };
    open(true).or_else(|_| open(false)).ok().map(OwnedFd::from)
}

/// Every kind of descriptor this suite makes without special hardware, and without unsafe code.
fn every_kind() -> Vec<(String, OwnedFd)> {
    use AddressFamily as F;
    use SocketType as T;
    let mut kinds: Vec<(String, OwnedFd)> = Vec::new();
    let mut add = |name: &str, descriptor: Option<OwnedFd>| match descriptor {
        Some(descriptor) => kinds.push((name.to_string(), descriptor)),
        None => eprintln!("{name}: cannot make"),
    };
    let file = |path: &str| std::fs::File::open(path).ok().map(OwnedFd::from);
    add("file", file("/proc/self/exe"));
    add("dir", file("/tmp"));
    add("proc-status", file("/proc/self/status"));
    add("sysfs", file("/sys/kernel/mm/transparent_hugepage/enabled"));
    for path in [
        "/dev/null", "/dev/zero", "/dev/full", "/dev/urandom", "/dev/ptmx", "/dev/net/tun", "/dev/fuse",
        "/dev/vhost-net", "/dev/vhost-vsock", "/dev/loop-control", "/dev/mapper/control", "/dev/userfaultfd",
        "/dev/uinput", "/dev/rfkill", "/dev/autofs", "/dev/btrfs-control", "/dev/loop0", "/dev/vfio/vfio",
        "/dev/udmabuf", "/dev/vsock",
    ] {
        add(path, device(path));
    }
    if let Ok(master) = rustix::pty::openpt(rustix::pty::OpenptFlags::RDWR | rustix::pty::OpenptFlags::NOCTTY) {
        let _ = rustix::pty::grantpt(&master);
        let _ = rustix::pty::unlockpt(&master);
        let name = rustix::pty::ptsname(&master, Vec::new()).ok();
        add("pty-slave", name.and_then(|name| device(name.to_str().ok()?)));
        add("pty-master", Some(master));
    }
    let (pipe_read, pipe_write) = std::io::pipe().expect("a pipe");
    add("pipe-read", Some(pipe_read.into()));
    add("pipe-write", Some(pipe_write.into()));
    add("eventfd", rustix::event::eventfd(0, rustix::event::EventfdFlags::empty()).ok());
    add("epoll", rustix::event::epoll::create(rustix::event::epoll::CreateFlags::empty()).ok());
    add("memfd", rustix::fs::memfd_create("x", rustix::fs::MemfdFlags::empty()).ok());
    add(
        "timerfd",
        rustix::time::timerfd_create(rustix::time::TimerfdClockId::Monotonic, rustix::time::TimerfdFlags::empty()).ok(),
    );
    add("inotify", rustix::fs::inotify::init(rustix::fs::inotify::CreateFlags::empty()).ok());
    let fanotify = nix::sys::fanotify::Fanotify::init(
        nix::sys::fanotify::InitFlags::empty(),
        nix::sys::fanotify::EventFFlags::O_RDONLY,
    );
    add("fanotify", fanotify.ok().map(OwnedFd::from));
    add("pidfd", rustix::process::pidfd_open(rustix::process::getpid(), rustix::process::PidfdFlags::empty()).ok());
    let signals = nix::sys::signalfd::SignalFd::new(&nix::sys::signal::SigSet::empty());
    add("signalfd", signals.ok().and_then(|signals| rustix::io::dup(&signals).ok()));
    add("unix-stream", sock(F::UNIX, T::STREAM, None));
    add("unix-dgram", sock(F::UNIX, T::DGRAM, None));
    add("unix-seqpacket", sock(F::UNIX, T::SEQPACKET, None));
    add("netlink-route", sock(F::NETLINK, T::RAW, None));
    add("netlink-generic", numbered(16, 3, 16));
    add("netlink-audit", numbered(16, 3, 9));
    add("packet-raw", sock(F::PACKET, T::RAW, None));
    add("packet-dgram", sock(F::PACKET, T::DGRAM, None));
    add("alg", numbered(38, 5, 0));
    add("vsock", sock(F::VSOCK, T::STREAM, None));
    add("xdp", numbered(44, 3, 0));
    add("tipc", numbered(30, 4, 0));
    add("rds", numbered(21, 5, 0));
    add("kcm", numbered(41, 2, 0));
    add("can-raw", numbered(29, 3, 1));
    add("llc", numbered(26, 2, 0));
    add("smc", numbered(43, 1, 0));
    add("qrtr", numbered(42, 2, 0));
    add("mctp", numbered(45, 2, 0));
    add("phonet", numbered(35, 2, 0));
    add("bluetooth-l2cap", numbered(31, 5, 0));
    add("bluetooth-hci", numbered(31, 3, 1));
    add("ieee802154", numbered(36, 3, 0));
    add("key", numbered(15, 3, 2));
    add("inet-sctp", numbered(2, 1, 132));
    add("inet6-sctp-seq", numbered(10, 5, 132));
    add("inet-l2tp", numbered(2, 2, 115));
    add("inet-smc", numbered(2, 1, 256));
    add("inet-ping", sock(F::INET, T::DGRAM, Some(ipproto::ICMP)));
    add("inet-udp", sock(F::INET, T::DGRAM, Some(ipproto::UDP)));
    add("inet6-udplite", sock(F::INET6, T::DGRAM, Some(ipproto::UDPLITE)));
    add("inet-raw-udp", sock(F::INET, T::RAW, Some(ipproto::UDP)));
    add("inet-raw-tcp", sock(F::INET, T::RAW, Some(ipproto::TCP)));
    add("inet6-raw-tcp", sock(F::INET6, T::RAW, Some(ipproto::TCP)));
    add("inet-raw-mptcp", numbered(2, 3, 262));
    add("inet-tcp-unconnected", sock(F::INET, T::STREAM, Some(ipproto::TCP)));
    add("inet-mptcp-unconnected", sock(F::INET, T::STREAM, Some(ipproto::MPTCP)));
    if let Some((accepted, connecting, listener)) = pair(F::INET, ipproto::TCP) {
        let tls = nix::sys::socket::setsockopt(&accepted, nix::sys::socket::sockopt::TcpUlp::default(), &b"tls".to_vec());
        add("inet-tcp-ktls", tls.ok().map(|()| accepted));
        add("inet-tcp-listener", Some(listener));
        add("inet-tcp-connecting", Some(connecting));
    }
    kinds
}

type Operation = Box<dyn Fn(BorrowedFd<'_>, u64) -> Result<(), Errno> + Send + Sync>;

/// Every method, with the decoy's address in every 16th 8-byte place of each value that can hold one.
fn operations() -> Vec<Operation> {
    let mut operations: Vec<Operation> = Vec::new();
    for &option in PlainOption::ALL {
        let length = option.value().exact_length().unwrap_or(64);
        operations.push(Box::new(move |socket, _| L.get_option(socket, option, &mut vec![0u8; length]).map(drop)));
        if option.value() != Value::Filled {
            let mut valid = valid_value(option, false, 7);
            valid.resize(length, 0);
            operations.push(Box::new(move |socket, decoy| {
                let mut value = valid.clone();
                for at in (8..length.saturating_sub(7)).step_by(16) {
                    put(&mut value, at, &decoy.to_ne_bytes());
                }
                L.set_option(socket, option, &value)
            }));
        }
    }
    for count in COUNTS {
        operations.push(Box::new(move |socket, _| L.count_queued(socket, count).map(drop)));
    }
    operations.push(Box::new(|socket, decoy| {
        let mut query = [0u8; AUTHENTICATION_KEY_QUERY];
        for at in (0..AUTHENTICATION_KEY_QUERY - 7).step_by(8) {
            put(&mut query, at, &decoy.to_ne_bytes());
        }
        L.get_authentication_key(socket, &mut query)
    }));
    operations.push(Box::new(|socket, decoy| {
        let mut group = storage(IpAddr::V4(Ipv4Addr::new(232, 9, 9, 9)));
        put(&mut group, 8, &decoy.to_ne_bytes());
        let sources = [group, group];
        let level = MulticastLevel::Ipv4;
        L.set_source_filter(socket, &SourceFilterOptions { level, interface: 1, group, mode: uapi::MCAST_INCLUDE, sources: &sources })
    }));
    operations.push(Box::new(|socket, _| {
        let (mut subflows, mut information, mut connection) = ([[0u8; 24]; 2], [[0u8; 64]; 2], [0u8; 16]);
        L.get_multipath_full_information(socket, &mut connection, &mut subflows[..], &mut information[..]).map(drop)
    }));
    operations
}

/// Third check (d): fd 0 swapped between a TCP connection and each kind of descriptor while every method is called on
/// it, with an untouched decoy's address planted in the values; any kernel access to the decoy faults and shows.
#[test]
fn a_swapped_descriptor_reaches_no_memory_outside_the_scratch() {
    let decoy = MmapOptions::new().len(PAGE).map_anon().expect("a decoy");
    let decoy_address = decoy.as_ptr().addr() as u64 + 128;
    let stdin = std::io::stdin();
    let saved = rustix::io::dup(&stdin).expect("stdin is kept");
    let tcp = pair(AddressFamily::INET, ipproto::TCP).expect("a TCP connection");
    let kinds = every_kind();
    let operations = operations();
    eprintln!("{} kinds, {} operations, {RACED_CALLS} calls a kind", kinds.len(), operations.len());
    let (mut failures, mut all) = (Vec::new(), BTreeMap::<&str, u64>::new());
    for (name, other) in &kinds {
        let ((), tally) = traced(1 << 16, || {
            let stop = AtomicBool::new(false);
            std::thread::scope(|scope| {
                scope.spawn(|| {
                    while !stop.load(Ordering::Relaxed) {
                        let _ = rustix::stdio::dup2_stdin(&tcp.0);
                        let _ = rustix::stdio::dup2_stdin(other);
                    }
                });
                for call in 0..RACED_CALLS {
                    let _ = operations[call % operations.len()](stdin.as_fd(), decoy_address);
                }
                stop.store(true, Ordering::Relaxed);
            });
        });
        for (class, count) in &tally.classes {
            *all.entry(class).or_default() += count;
        }
        for line in tally.outside() {
            failures.push(format!("{name}: {line}"));
        }
    }
    rustix::stdio::dup2_stdin(&saved).expect("stdin is put back");
    eprintln!("{} kinds swapped: kernel faults by class {all:?}", kinds.len());
    assert!(failures.is_empty(), "{failures:#?}");
}
