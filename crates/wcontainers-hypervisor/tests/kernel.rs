//! N3's kernel and loop inside the machine: the test program `native-kernel` takes a container's files and
//! start through the link and runs its first process to its end in the kernel on the native CPU side, with the
//! runtime's loop and the kernel test crate's policy, the host's services through the link. The same kernel,
//! loop and policy run the same container on the WebAssembly output's CPU side on the host, through the same
//! code over a link in memory. The wait status, the number of turns and every call of the host's services
//! must agree.
//!
//! `WCONTAINERS_KERNEL_SEED` seeds the policy and the host's random bytes.

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

    use linux_raw_sys::general::{S_IRGRP, S_IROTH, S_IRWXU, S_IXGRP, S_IXOTH};

    use wcontainers_abi::{BackendRequestId, ConnectionId, Host, HostError, Stream, Warning};
    use wcontainers_arm64::Arm64;
    use wcontainers_hypervisor::{
        Exit as VcpuExit, Loaded, Memory, Register, VirtualCpu, VirtualMachine, boot,
        link as link_page, routines, translate,
    };
    use wcontainers_inside::container::{self, File, FileKind, Starting};
    use wcontainers_inside::link::{self, Fatal, Link, Parts, Ring};
    use wcontainers_inside::{host, routines as inside_routines, translation};
    use wcontainers_kernel_test_policy::{TestPolicy, Warned};
    use wcontainers_runtime::{Ending, Kernel, drive};
    use wcontainers_wasmtime::{Compiler, CpuSide, LookAhead, Optimise};
    use zerocopy::FromZeros;

    use crate::common::{
        CLibrary, KERNEL_TEST_GUEST, MEMORY, ONE_CPU, be_signed, build, compile, load_average,
        machine, setting,
    };
    use crate::protocol::{self, Ended, PolicySeeds};

    /// The test program inside.
    const PROGRAM: &str = "native-kernel";

    /// The bar's first program, from the kernel test crate.
    const HELLO: &str = "hello";

    const SEED_VARIABLE: &str = "WCONTAINERS_KERNEL_SEED";
    const DEFAULT_SEED: u64 = 0x6b65_726e;

    /// The clocks the test host reads.
    const MONOTONIC_NANOSECONDS: u64 = 1_234_567;
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

    /// What one side's run did.
    struct Outcome {
        status: i32,
        turns: u64,
        calls: Vec<String>,
        /// The CPU side's slow paths and the policy's refusals.
        warnings: Vec<Warned>,
    }

    pub fn main() {
        be_signed();
        let seed = setting(SEED_VARIABLE, DEFAULT_SEED);
        let started = Instant::now();
        let hello = compile(KERNEL_TEST_GUEST, HELLO, CLibrary::Without);
        let path = format!("/{HELLO}").into_bytes();
        let files = vec![File {
            path: path.clone(),
            mode: S_IRWXU | S_IRGRP | S_IXGRP | S_IROTH | S_IXOTH,
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
        for (side, outcome) in [("native", &native), ("WebAssembly", &wasm)] {
            println!(
                "{HELLO} on the {side} side: status {:#x} after {} turns, {} host calls, warned {:?}",
                outcome.status,
                outcome.turns,
                outcome.calls.len(),
                outcome.warnings
            );
        }
        assert_eq!(native.status, wasm.status, "the wait status");
        assert_eq!(native.turns, wasm.turns, "the turns");
        assert_eq!(native.calls, wasm.calls, "the calls of the host's services");
        println!(
            "kernel: ran alike to the end; seed {seed:#x}; in {:?}, load {}",
            started.elapsed(),
            load_average()
        );
    }

    /// Runs the container in the kernel inside the machine.
    fn native(seed: u64, files: &[File], starting: &Starting) -> Outcome {
        let program = build(PROGRAM);
        let (machine, memory, loaded, start) = machine(&program, MEMORY, ONE_CPU);
        let mut cpu = machine.cpu().expect("a virtual CPU");
        boot(&mut cpu, &memory, &loaded, 0, start).expect("booting");
        let mut host = Recorded::new(seed);
        let mut parts = Parts::default();
        let mut files = files.iter().cloned();
        let mut warnings = Vec::new();
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
                    protocol::Request::Policy => link.put(&PolicySeeds {
                        seed,
                        wall_start: REALTIME_NANOSECONDS,
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
                        let Ended { status, turns } = link.get();
                        return Outcome {
                            status: status as i32,
                            turns,
                            calls: host.calls,
                            warnings,
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

    /// Runs the container in the same kernel, loop and policy on the host, on the WebAssembly output's CPU side.
    fn wasm(seed: u64, files: &[File], starting: &Starting) -> Outcome {
        let compiler = Compiler::<Arm64>::new(None);
        let mut cpu = CpuSide::new(compiler, LookAhead::None, Optimise::Speed, ());
        let mut policy = TestPolicy::new(Recorded::new(seed), seed, REALTIME_NANOSECONDS);
        let mut kernel = Kernel::new();
        let mut ring = Loopback {
            link: Box::new(Link::new_zeroed()),
            parts: Parts::default(),
            files: files.iter().cloned(),
            starting,
        };
        container::start(&mut ring, &mut kernel, &mut cpu, &mut policy)
            .expect("files the kernel takes");
        let status = match drive(&mut kernel, &mut cpu, &mut policy) {
            Ok(Ending::Finished { status }) => status,
            Ok(Ending::Ended(never)) => match never {},
            Err(never) => match never {},
        };
        Outcome {
            status,
            turns: policy.turns,
            calls: policy.services.calls,
            warnings: policy.warnings,
        }
    }
}
