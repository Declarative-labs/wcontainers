//! A container run to its end in the kernel inside the machine (the test program `native-kernel`), and the
//! same container in the same kernel, loop and policy on the host on the WebAssembly output's CPU side, with a
//! host of fixed clocks and seeded random bytes that records every call.

#![allow(dead_code)]

use linux_raw_sys::general::{S_IRGRP, S_IROTH, S_IRWXU, S_IXGRP, S_IXOTH};
use wcontainers_abi::{BackendRequestId, ConnectionId, Host, HostError, Stream, Warning};
use wcontainers_arm64::Arm64;
use wcontainers_hypervisor::{
    Exit as VcpuExit, Loaded, Memory, Register, VirtualCpu, VirtualMachine, boot,
    link as link_page, routines, translate::Translator,
};
use wcontainers_inside::container::{self, File, FileKind, Monitor, Starting};
use wcontainers_inside::link::{self, Fatal, Link, Parts, Ring};
use wcontainers_inside::{host, routines as inside_routines, translation};
use wcontainers_kernel::FileRecord;
use wcontainers_kernel_test_policy::{TestPolicy, Warned};
use wcontainers_runtime::{Ending, Kernel, drive};
use wcontainers_wasmtime::{Compiler, CpuSide, LookAhead, Optimise};
use zerocopy::FromZeros;

use crate::common::{MEMORY, ONE_CPU, build, look_ahead, machine, setting};
use crate::protocol::{self, Ended, PolicySeeds};

/// The test program inside.
const PROGRAM: &str = "native-kernel";

/// Seeds the policy and the host's random bytes.
pub const SEED_VARIABLE: &str = "WCONTAINERS_KERNEL_SEED";
pub const DEFAULT_SEED: u64 = 0x6b65_726e;

/// How many seeds a run takes, the first and each one after it.
pub const SEEDS_VARIABLE: &str = "WCONTAINERS_KERNEL_SEEDS";
pub const DEFAULT_SEEDS: u64 = 1;

/// The seeds a run takes: the first from [`SEED_VARIABLE`], then the ones after it, as many as
/// [`SEEDS_VARIABLE`] says.
pub fn seeds() -> Vec<u64> {
    let first = setting(SEED_VARIABLE, DEFAULT_SEED);
    (0..setting(SEEDS_VARIABLE, DEFAULT_SEEDS))
        .map(|i| first.wrapping_add(i))
        .collect()
}

/// The clocks the test host reads.
const MONOTONIC_NANOSECONDS: u64 = 1_234_567;
pub const REALTIME_NANOSECONDS: u64 = 1_763_500_000_000_000_000;

/// A host with fixed clocks and seeded random bytes, recording every call and keeping each stream's output,
/// with no network or backend.
pub struct Recorded {
    pub calls: Vec<String>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    random: fastrand::Rng,
}

impl Recorded {
    pub fn new(seed: u64) -> Recorded {
        Recorded {
            calls: Vec::new(),
            stdout: Vec::new(),
            stderr: Vec::new(),
            random: fastrand::Rng::with_seed(seed),
        }
    }

    fn absent(&mut self, call: String) -> Result<(), HostError> {
        self.calls.push(call);
        Err(HostError::NotSupported)
    }
}

impl Host for Recorded {
    fn monotonic_nanoseconds(&mut self) -> u64 {
        self.calls.push("monotonic".into());
        MONOTONIC_NANOSECONDS
    }
    fn realtime_nanoseconds(&mut self) -> u64 {
        self.calls.push("realtime".into());
        REALTIME_NANOSECONDS
    }
    fn random(&mut self, into: &mut [u8]) {
        self.random.fill(into);
        self.calls.push(format!("random {into:02x?}"));
    }
    fn warn(&mut self, warning: Warning<'_>) {
        self.calls.push(format!("warn {warning:?}"));
    }
    fn backend_send(&mut self, request: BackendRequestId, message: &[u8]) -> Result<(), HostError> {
        self.absent(format!("backend {request:?} {message:02x?}"))
    }
    fn output(&mut self, stream: Stream, bytes: &[u8]) {
        self.calls.push(format!(
            "output {stream:?} {:?}",
            String::from_utf8_lossy(bytes)
        ));
        match stream {
            Stream::Output => &mut self.stdout,
            Stream::Error => &mut self.stderr,
        }
        .extend_from_slice(bytes);
    }
    fn connect(
        &mut self,
        connection: ConnectionId,
        address: &[u8],
        port: u16,
        stream: bool,
    ) -> Result<(), HostError> {
        self.absent(format!(
            "connect {connection:?} {address:?} {port} {stream}"
        ))
    }
    fn send(&mut self, connection: ConnectionId, bytes: &[u8]) -> Result<usize, HostError> {
        self.absent(format!("send {connection:?} {bytes:02x?}"))
            .map(|()| 0)
    }
    fn receive(&mut self, connection: ConnectionId, into: &mut [u8]) -> Result<usize, HostError> {
        self.absent(format!("receive {connection:?} {}", into.len()))
            .map(|()| 0)
    }
    fn finish(&mut self, connection: ConnectionId) {
        self.calls.push(format!("finish {connection:?}"));
    }
    fn close(&mut self, connection: ConnectionId) {
        self.calls.push(format!("close {connection:?}"));
    }
    fn look_up(&mut self, request: u64, name: &str) -> Result<(), HostError> {
        self.absent(format!("look up {request} {name}"))
    }
    fn listen(&mut self, listener: ConnectionId, port: u16) -> Result<(), HostError> {
        self.absent(format!("listen {listener:?} {port}"))
    }
    fn accept(
        &mut self,
        listener: ConnectionId,
        connection: ConnectionId,
    ) -> Result<(), HostError> {
        self.absent(format!("accept {listener:?} {connection:?}"))
    }
}

/// What one side's run did.
pub struct Outcome {
    pub status: i32,
    pub turns: u64,
    /// The host's record of the services called and the output.
    pub host: Recorded,
    /// The CPU side's slow paths and the policy's refusals.
    pub warnings: Vec<Warned>,
    /// How many times the virtual machine gave its CPU back to the monitor; none on the host.
    pub exits: u64,
    /// The root's files when the container ended.
    pub root: Vec<FileRecord>,
    /// How many times its threads stopped and the hash of their positions, natively.
    pub positions: Option<(u64, u64)>,
}

/// A container of one program at the root, started with `arguments` after its path.
pub fn one_program(name: &str, image: Vec<u8>, arguments: &[String]) -> (Vec<File>, Starting) {
    let path = format!("/{name}").into_bytes();
    let files = vec![File {
        path: path.clone(),
        mode: S_IRWXU | S_IRGRP | S_IXGRP | S_IROTH | S_IXOTH,
        uid: 0,
        gid: 0,
        mtime_seconds: 0,
        kind: FileKind::Regular(image),
    }];
    let starting = Starting {
        path: path.clone(),
        arguments: std::iter::once(path)
            .chain(arguments.iter().map(|a| a.clone().into_bytes()))
            .collect(),
        environment: vec![b"PATH=/".to_vec()],
        working_directory: b"/".to_vec(),
        hostname: b"native".to_vec(),
        groups: Vec::new(),
        backend: None,
    };
    (files, starting)
}

/// Each warning's reason with its last count.
pub fn summary(warnings: &[Warned]) -> std::collections::BTreeMap<&str, u64> {
    warnings
        .iter()
        .map(|w| (w.reason.as_str(), w.count))
        .collect()
}

/// Runs the container in the kernel inside the machine, its wall clock starting at `wall_start`, at look-ahead
/// none and at the bound [`look_ahead`] gives, which must agree in everything the guest and its host see and in
/// the count at every stop; gives the bounded run's outcome.
pub fn native(seed: u64, wall_start: u64, files: &[File], starting: &Starting) -> Outcome {
    native_alike(seed, wall_start, None, files, starting)
}

/// As [`native`], every turn ending after one counted event, so that the threads stop at every one.
pub fn native_at_every_event(
    seed: u64,
    wall_start: u64,
    files: &[File],
    starting: &Starting,
) -> Outcome {
    native_alike(seed, wall_start, Some(1), files, starting)
}

/// As [`native`], every turn ending after `turn_events` counted events when that is given.
fn native_alike(
    seed: u64,
    wall_start: u64,
    turn_events: Option<u64>,
    files: &[File],
    starting: &Starting,
) -> Outcome {
    let at = |look_ahead| {
        native_at(
            program(),
            look_ahead,
            Policy {
                seed,
                wall_start,
                turn_events,
            },
            files,
            starting,
        )
    };
    let none = at(LookAhead::None);
    let bounded = look_ahead();
    let outcome = at(bounded);
    let what = |o: &Outcome| {
        (
            o.status,
            o.turns,
            o.positions,
            o.host.calls.clone(),
            o.host.stdout.clone(),
            o.host.stderr.clone(),
            o.root.clone(),
        )
    };
    assert_eq!(
        what(&none),
        what(&outcome),
        "look-ahead none and {bounded:?} differ"
    );
    outcome
}

/// The image of the test program inside, built once.
pub fn program() -> &'static [u8] {
    static PROGRAM_IMAGE: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();
    PROGRAM_IMAGE.get_or_init(|| build(PROGRAM))
}

/// What the test crate's policy inside is made from: its seed, the wall clock's reading at the start, and the
/// counted events after which every turn ends, if not the schedule's limits.
#[derive(Clone, Copy)]
pub struct Policy {
    pub seed: u64,
    pub wall_start: u64,
    pub turn_events: Option<u64>,
}

/// Runs the container in the kernel inside the machine of the test program's image `program` at `look_ahead`,
/// with the policy made from `policy`.
pub fn native_at(
    program: &[u8],
    look_ahead: LookAhead,
    policy: Policy,
    files: &[File],
    starting: &Starting,
) -> Outcome {
    let Policy {
        seed,
        wall_start,
        turn_events,
    } = policy;
    let (machine, memory, loaded, start) = machine(program, MEMORY, ONE_CPU);
    let mut cpu = machine.cpu().expect("a virtual CPU");
    boot(&mut cpu, &memory, &loaded, 0, start).expect("booting");
    let mut host = Recorded::new(seed);
    let mut parts = Parts::default();
    let mut files = files.iter().cloned();
    let mut monitor = Monitor {
        files: &mut files,
        starting,
        root: Vec::new(),
    };
    let mut warnings = Vec::new();
    let mut exits = 0;
    let mut translator = Translator::new(look_ahead, None);
    loop {
        exits += 1;
        match cpu.run().expect("running") {
            VcpuExit::Doorbell => {}
            other => panic!(
                "the inside stopped: {other:?} at {:#x}",
                cpu.get(Register::Pc).unwrap_or_default()
            ),
        }
        let link = link_of(&memory, &loaded);
        let Some(bytes) = parts.request(link) else {
            continue;
        };
        let word = link.head.request;
        if let Some(request) = link::Request::from_word(word) {
            match request {
                link::Request::Print => {
                    println!("inside: {}", String::from_utf8_lossy(link.data()))
                }
                link::Request::Fatal => {
                    let Fatal { esr, elr, far } = link.get();
                    panic!("the inside faulted: ESR {esr:#x} ELR {elr:#x} FAR {far:#x}");
                }
                request => panic!("no {request:?} here"),
            }
        } else if translation::Request::from_word(word).is_some() {
            translator.answer(link);
        } else if inside_routines::Request::from_word(word).is_some() {
            routines::answer(link, &memory);
        } else if let Some(request) = host::Request::from_word(word) {
            let answer = host::answer(request, link, &bytes, &mut host);
            parts.answer(link, answer);
        } else if let Some(request) = container::Request::from_word(word) {
            let answer = monitor.answer(request, link, &bytes);
            parts.answer(link, answer);
        } else {
            match protocol::Request::from_word(word).expect("a request of the set") {
                protocol::Request::Policy => link.put(&PolicySeeds {
                    seed,
                    wall_start,
                    turn_events: turn_events.unwrap_or_default(),
                }),
                protocol::Request::Warned => {
                    let protocol::Warned { count, detail } = link.get();
                    warnings.push(Warned {
                        reason: String::from_utf8_lossy(link.data()).into_owned(),
                        count,
                        detail,
                    });
                }
                protocol::Request::Ended => {
                    let Ended {
                        status,
                        turns,
                        stops,
                        positions,
                    } = link.get();
                    return Outcome {
                        status: status as i32,
                        turns,
                        host,
                        warnings,
                        exits,
                        root: monitor.root,
                        positions: Some((stops, positions)),
                    };
                }
            }
        }
    }
}

fn link_of(memory: &Memory, loaded: &Loaded) -> &'static mut Link {
    link_page(memory, loaded, 0)
}

/// A link in memory whose requests the container's half answers at once.
struct Loopback<'a> {
    link: Box<Link>,
    parts: Parts,
    monitor: Monitor<'a>,
}

impl Ring for Loopback<'_> {
    fn link(&mut self) -> &mut Link {
        &mut self.link
    }
    fn ring(&mut self) {
        let Some(carried) = self.parts.request(&mut self.link) else {
            return;
        };
        let request =
            container::Request::from_word(self.link.head.request).expect("a container request");
        let answer = self.monitor.answer(request, &mut self.link, &carried);
        self.parts.answer(&mut self.link, answer);
    }
}

/// Runs the container in the same kernel, loop and policy on the host, on the WebAssembly output's CPU side.
pub fn wasm(seed: u64, wall_start: u64, files: &[File], starting: &Starting) -> Outcome {
    let compiler = Compiler::<Arm64>::new(None);
    let mut cpu = CpuSide::new(compiler, LookAhead::None, Optimise::Speed, ());
    let mut policy = TestPolicy::new(Recorded::new(seed), seed, wall_start);
    let mut kernel = Kernel::new();
    let mut files = files.iter().cloned();
    let mut ring = Loopback {
        link: Box::new(Link::new_zeroed()),
        parts: Parts::default(),
        monitor: Monitor {
            files: &mut files,
            starting,
            root: Vec::new(),
        },
    };
    container::start(&mut ring, &mut kernel, &mut cpu, &mut policy)
        .expect("files the kernel takes");
    let status = match drive(&mut kernel, &mut cpu, &mut policy) {
        Ok(Ending::Finished { status }) => status,
        Ok(Ending::Ended(never)) => match never {},
        Err(never) => match never {},
    };
    container::send_root(&mut ring, &kernel, &mut cpu);
    Outcome {
        status,
        turns: policy.turns,
        host: policy.services,
        warnings: policy.warnings,
        exits: 0,
        root: ring.monitor.root,
        positions: None,
    }
}
