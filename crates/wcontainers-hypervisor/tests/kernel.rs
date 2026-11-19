//! N3's kernel inside the machine: the test program `native-kernel` takes a container's files and start
//! through the link and starts its first process in the kernel on the native CPU side, with the host's
//! services through the link. The same kernel starts the same container on the WebAssembly output's CPU side
//! on the host, through the same code over a link in memory. What the kernel runs next, the first thread's
//! registers and every call the kernel makes of its host must agree.
//!
//! `WCONTAINERS_KERNEL_SEED` seeds the host's random bytes.

#[cfg(target_os = "macos")]
#[path = "../../wcontainers-inside/src/bin/native-kernel/protocol.rs"]
mod protocol;

#[cfg(target_os = "macos")]
mod common;

#[cfg(target_os = "macos")]
fn main() {
    macos::main();
}

#[cfg(not(target_os = "macos"))]
fn main() {
    println!("the kernel test needs Hypervisor.framework; nothing to run on this host");
}

#[cfg(target_os = "macos")]
mod macos {
    use std::time::Instant;

    use wcontainers_abi::{
        BackendRequestId, ConnectionId, Cpu, Host, HostError, RegisterBlock, Stream, Warning,
    };
    use wcontainers_arm64::Arm64;
    use wcontainers_hypervisor::{
        Exit as VcpuExit, Loaded, Memory, Register, VirtualCpu, VirtualMachine, boot,
        link as link_page, routines, translate,
    };
    use wcontainers_inside::container::{self, File, FileKind, Starting};
    use wcontainers_inside::link::{self, Fatal, Link, Parts, Ring};
    use wcontainers_inside::{host, routines as inside_routines, translation};
    use wcontainers_runtime::Kernel;
    use wcontainers_wasmtime::{Compiler, CpuSide, LookAhead, Optimise};
    use zerocopy::{FromBytes, FromZeros};

    use crate::common::{
        CLibrary, KERNEL_TEST_GUEST, MEMORY, ONE_CPU, be_signed, build, compile, load_average,
        machine, setting,
    };
    use crate::protocol::{self, Started};

    /// The test program inside.
    const PROGRAM: &str = "native-kernel";

    /// The bar's first program, from the kernel test crate.
    const HELLO: &str = "hello";

    const SEED_VARIABLE: &str = "WCONTAINERS_KERNEL_SEED";
    const DEFAULT_SEED: u64 = 0x5eed;

    /// The clocks the test host reads.
    const MONOTONIC_NANOSECONDS: u64 = 1_000_000;
    const REALTIME_NANOSECONDS: u64 = 1_763_500_000_000_000_000;

    /// A host with fixed clocks and seeded random bytes, recording every call, with no network or backend.
    struct Recorded {
        calls: Vec<String>,
        random: fastrand::Rng,
    }

    impl Recorded {
        fn new(seed: u64) -> Recorded {
            Recorded {
                calls: Vec::new(),
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
        fn backend_send(
            &mut self,
            request: BackendRequestId,
            message: &[u8],
        ) -> Result<(), HostError> {
            self.absent(format!("backend {request:?} {message:02x?}"))
        }
        fn output(&mut self, stream: Stream, bytes: &[u8]) {
            self.calls.push(format!(
                "output {stream:?} {:?}",
                String::from_utf8_lossy(bytes)
            ));
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
        fn receive(
            &mut self,
            connection: ConnectionId,
            into: &mut [u8],
        ) -> Result<usize, HostError> {
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

    /// What one side's kernel did at the start.
    struct Outcome {
        started: Started,
        block: RegisterBlock,
        calls: Vec<String>,
    }

    pub fn main() {
        be_signed();
        let seed = setting(SEED_VARIABLE, DEFAULT_SEED);
        let started = Instant::now();
        let hello = compile(KERNEL_TEST_GUEST, HELLO, CLibrary::Without);
        let path = format!("/{HELLO}").into_bytes();
        let files = vec![File {
            path: path.clone(),
            mode: 0o755,
            uid: 0,
            gid: 0,
            mtime_seconds: 0,
            kind: FileKind::Regular(hello),
        }];
        let starting = Starting {
            path: path.clone(),
            arguments: vec![path],
            environment: vec![b"PATH=/".to_vec()],
            working_directory: b"/".to_vec(),
            hostname: b"native".to_vec(),
            groups: Vec::new(),
            backend: None,
        };
        let native = native(seed, &files, &starting);
        let wasm = wasm(seed, &files, &starting);
        println!(
            "{HELLO}: native {:?}, WebAssembly {:?}; {} host calls; seed {seed:#x}",
            native.started,
            wasm.started,
            native.calls.len()
        );
        assert_eq!(
            (
                native.started.kind,
                native.started.thread,
                native.started.status
            ),
            (wasm.started.kind, wasm.started.thread, wasm.started.status),
            "what the kernel runs next"
        );
        assert_eq!(
            native.started.kind,
            protocol::NextKind::Run as u64,
            "the first thread runs"
        );
        assert_eq!(native.calls, wasm.calls, "the kernel's calls of its host");
        // The stack's top is placed from each side's ceiling until the kernel places low first (K2).
        assert_eq!(
            RegisterBlock {
                sp: 0,
                ..native.block
            },
            RegisterBlock {
                sp: 0,
                ..wasm.block
            },
            "the first thread's registers but its stack pointer"
        );
        println!(
            "kernel: started alike, first pc {:#x}, stack pointers {:#x} and {:#x}; in {:?}, load {}",
            native.block.pc,
            native.block.sp,
            wasm.block.sp,
            started.elapsed(),
            load_average()
        );
    }

    /// Starts the container in the kernel inside the machine.
    fn native(seed: u64, files: &[File], starting: &Starting) -> Outcome {
        let program = build(PROGRAM);
        let (machine, memory, loaded, start) = machine(&program, MEMORY, ONE_CPU);
        let mut cpu = machine.cpu().expect("a virtual CPU");
        boot(&mut cpu, &memory, &loaded, 0, start).expect("booting");
        let mut host = Recorded::new(seed);
        let mut parts = Parts::default();
        let mut files = files.iter().cloned();
        loop {
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
                translate::translate(link).expect("a piece fits the link");
            } else if inside_routines::Request::from_word(word).is_some() {
                routines::answer(link, &memory);
            } else if let Some(request) = host::Request::from_word(word) {
                let answer = host::answer(request, link, &bytes, &mut host);
                parts.answer(link, answer);
            } else if let Some(request) = container::Request::from_word(word) {
                let answer = container::answer(request, link, &mut files, starting);
                parts.answer(link, answer);
            } else {
                match protocol::Request::from_word(word).expect("a request of the set") {
                    protocol::Request::Started => {
                        return Outcome {
                            started: link.get(),
                            block: RegisterBlock::read_from_bytes(link.data())
                                .expect("a register block"),
                            calls: host.calls,
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
        files: std::iter::Cloned<std::slice::Iter<'a, File>>,
        starting: &'a Starting,
    }

    impl Ring for Loopback<'_> {
        fn link(&mut self) -> &mut Link {
            &mut self.link
        }
        fn ring(&mut self) {
            let Some(_) = self.parts.request(&mut self.link) else {
                return;
            };
            let request =
                container::Request::from_word(self.link.head.request).expect("a container request");
            let answer = container::answer(request, &mut self.link, &mut self.files, self.starting);
            self.parts.answer(&mut self.link, answer);
        }
    }

    /// Starts the container in the same kernel on the host, on the WebAssembly output's CPU side.
    fn wasm(seed: u64, files: &[File], starting: &Starting) -> Outcome {
        let compiler = Compiler::<Arm64>::new(None);
        let mut cpu = CpuSide::new(compiler, LookAhead::None, Optimise::Speed, ());
        let mut host = Recorded::new(seed);
        let mut kernel = Kernel::new();
        let mut ring = Loopback {
            link: Box::new(Link::new_zeroed()),
            parts: Parts::default(),
            files: files.iter().cloned(),
            starting,
        };
        container::start(&mut ring, &mut kernel, &mut cpu, &mut host)
            .expect("files the kernel takes");
        let next = kernel.next();
        let mut block = RegisterBlock::ZERO;
        if let wcontainers_abi::Next::Run { thread, .. } = next {
            cpu.registers(thread, &mut block);
        }
        Outcome {
            started: Started::of(next),
            block,
            calls: host.calls,
        }
    }
}
