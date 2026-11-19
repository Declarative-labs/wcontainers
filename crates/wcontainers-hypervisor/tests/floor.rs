//! N1's bar: builds the floor's cases program (wcontainers-inside's `floor-cases`, never part of the monitor)
//! for the bare-metal target, boots it under the monitor, serves its link, makes the stops from another
//! thread, and checks that every case of the bar reported once and passed. It runs itself as a copy signed
//! with the hypervisor entitlement.
//!
//! The stop test is seeded: `WCONTAINERS_FLOOR_STOPS` sets how many stops it makes and
//! `WCONTAINERS_FLOOR_SEED` (decimal, or hexadecimal after `0x`) the seed of the waits between them.

#[cfg(target_os = "macos")]
#[path = "../../wcontainers-inside/src/bin/floor-cases/protocol.rs"]
mod protocol;

#[cfg(target_os = "macos")]
fn main() {
    macos::main();
}

#[cfg(not(target_os = "macos"))]
fn main() {
    println!("the floor test needs Hypervisor.framework; nothing to run on this host");
}

#[cfg(target_os = "macos")]
#[allow(deprecated)] // libc's Mach clock calls, which the test uses to time stops
mod macos {
    use std::collections::HashMap;
    use std::path::Path;
    use std::process::Command;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::{Duration, Instant};

    use wcontainers_hypervisor::{
        Exit, Host, Hypervisor, Loaded, Memory, Register, Stopper, VirtualCpu, VirtualMachine,
        boot, link as link_page, load,
    };
    use wcontainers_inside::link::{self, Fatal, GiveBack, Start};

    use crate::protocol::{
        self, Bytes, Case, CounterValue, Lower, Measurement, RaiseLater, Report, StartCpu,
        StopTest as StopRequest, Write,
    };

    /// The inside's package, which holds the cases program.
    const INSIDE: &str = "wcontainers-inside";

    /// The cases program.
    const PROGRAM: &str = "floor-cases";

    /// The bare-metal target of each host architecture the cases are built for.
    const TARGETS: &[(&str, &str)] = &[("aarch64", "aarch64-unknown-none")];

    /// The cargo profile the cases are built with.
    const PROFILE: &str = "release";

    /// Jobs the nested build may run.
    const JOBS: &str = "4";

    /// Settings of the test's environment that would build the bare-metal program for the host.
    const LEAKED: [&str; 2] = ["RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS"];

    /// Bytes of guest-physical memory the test reserves.
    const MEMORY: u64 = 1 << 30;

    /// Virtual CPUs the cases use.
    const CPUS: u64 = 2;

    /// The stop test waits between stops for a seeded time in this range of microseconds.
    const STOP_WAIT_US: std::ops::Range<u64> = 200..2000;

    /// The environment variable that sets how many stops the stop test makes.
    const STOPS_VARIABLE: &str = "WCONTAINERS_FLOOR_STOPS";

    /// The environment variable that sets the stop test's seed.
    const SEED_VARIABLE: &str = "WCONTAINERS_FLOOR_SEED";

    /// Stops the stop test makes unless told otherwise.
    const DEFAULT_STOPS: u64 = 300;

    /// The stop test's seed unless told otherwise.
    const DEFAULT_SEED: u64 = 0x9e37_79b9_7f4a_7c15;

    /// What starts a hexadecimal setting.
    const HEX_PREFIX: &str = "0x";

    /// The radix of a hexadecimal setting.
    const HEX_RADIX: u32 = 16;

    /// The stop test's settings.
    #[derive(Clone, Copy, Debug)]
    struct StopSettings {
        stops: u64,
        seed: u64,
    }

    /// A number from the environment variable `name`, or `default`.
    fn setting(name: &str, default: u64) -> u64 {
        let Ok(text) = std::env::var(name) else {
            return default;
        };
        let parsed = match text.strip_prefix(HEX_PREFIX) {
            Some(hex) => u64::from_str_radix(hex, HEX_RADIX),
            None => text.parse(),
        };
        parsed.unwrap_or_else(|e| panic!("{name}={text}: {e}"))
    }

    fn stop_settings() -> StopSettings {
        StopSettings {
            stops: setting(STOPS_VARIABLE, DEFAULT_STOPS),
            seed: setting(SEED_VARIABLE, DEFAULT_SEED),
        }
    }

    /// One machine and what the cases told it.
    struct Shared {
        machine: <Host as Hypervisor>::VirtualMachine,
        memory: Memory,
        loaded: Loaded,
        start: Start,
        stop_settings: StopSettings,
        reports: Mutex<Vec<(Option<Case>, bool, String)>>,
        measurements: Mutex<Vec<(Option<Case>, String)>>,
        raised_at: Mutex<HashMap<u64, u64>>,
        stop_delays: Mutex<Vec<f64>>,
        fatal: AtomicBool,
    }

    /// Set in the environment of the signed copy the test runs itself as.
    const SIGNED: &str = "WCONTAINERS_FLOOR_TEST_SIGNED";

    /// Hypervisor.framework refuses a binary without the hypervisor entitlement, which cargo's test binaries
    /// lack: the test signs a copy of itself ad hoc with the entitlement and runs that.
    fn run_signed() -> ! {
        let me = std::env::current_exe().expect("the test's own path");
        let copy = me.with_extension("signed");
        std::fs::copy(&me, &copy).expect("copying the test");
        let entitlements = concat!(env!("CARGO_MANIFEST_DIR"), "/hypervisor.entitlements");
        let signed = std::process::Command::new("codesign")
            .args(["-s", "-", "-f", "--entitlements", entitlements])
            .arg(&copy)
            .output()
            .expect("running codesign");
        assert!(
            signed.status.success(),
            "codesign: {}",
            String::from_utf8_lossy(&signed.stderr)
        );
        let status = std::process::Command::new(&copy)
            .args(std::env::args_os().skip(1))
            .env(SIGNED, "1")
            .status()
            .expect("running the signed test");
        std::process::exit(status.code().unwrap_or(1));
    }

    pub fn main() {
        if std::env::var_os(SIGNED).is_none() {
            run_signed();
        }
        let started = Instant::now();
        let program = build_cases();
        let built = started.elapsed();
        let settings = stop_settings();
        let shared = machine(&program, settings);
        serve(&shared, 0);
        let reports = shared.reports.lock().unwrap().clone();
        let measurements = shared.measurements.lock().unwrap().clone();
        let delays = shared.stop_delays.lock().unwrap().clone();
        let fatal = shared.fatal.load(Ordering::Relaxed);
        drop(shared);
        let mut failed = 0;
        for (case, passed, text) in &reports {
            println!("{} {case:?}: {text}", if *passed { "pass" } else { "FAIL" });
            failed += usize::from(!*passed);
        }
        for (case, text) in &measurements {
            println!("measured {case:?}: {text}");
        }
        print_delays(&delays, settings);
        println!("load {}", load_average());
        println!(
            "{} cases and {} measurements of {}, {failed} failed, in {:?} ({built:?} of it building the cases)",
            reports.len(),
            measurements.len(),
            Case::ALL.len(),
            started.elapsed()
        );
        assert!(!fatal, "the inside faulted");
        assert_eq!(failed, 0, "cases failed");
        let mut seen: Vec<Option<Case>> = reports
            .iter()
            .map(|(case, ..)| *case)
            .chain(measurements.iter().map(|(case, _)| *case))
            .collect();
        seen.sort_by_key(|case| case.map(|c| c as u64));
        let every: Vec<Option<Case>> = Case::ALL.iter().copied().map(Some).collect();
        assert_eq!(seen, every, "every case of the bar reports exactly once");
        assert_eq!(
            delays.len() as u64,
            settings.stops,
            "not every stop was seen"
        );
    }

    /// Builds the cases program for the host's bare-metal target and gives its ELF image.
    fn build_cases() -> Vec<u8> {
        let target = TARGETS
            .iter()
            .find(|(arch, _)| *arch == std::env::consts::ARCH)
            .map(|(_, target)| *target)
            .expect("a bare-metal target for this host");
        let target_dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(INSIDE);
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"))
            .with_file_name(INSIDE)
            .join("Cargo.toml");
        let mut cargo = Command::new(env!("CARGO"));
        cargo
            .args([
                "build",
                "--bin",
                PROGRAM,
                "--target",
                target,
                "--profile",
                PROFILE,
            ])
            .args(["--jobs", JOBS])
            .arg("--manifest-path")
            .arg(&manifest)
            .arg("--target-dir")
            .arg(&target_dir);
        for leaked in LEAKED {
            cargo.env_remove(leaked);
        }
        let status = cargo.status().expect("running cargo for the cases");
        assert!(status.success(), "building the cases failed");
        std::fs::read(target_dir.join(target).join(PROFILE).join(PROGRAM))
            .expect("the cases' image")
    }

    fn machine(program: &[u8], stop_settings: StopSettings) -> Arc<Shared> {
        let machine = Host::machine().expect("making the machine");
        let memory = Memory::reserve(MEMORY).expect("reserving memory");
        machine
            .map(
                std::ptr::NonNull::new(memory.host(wcontainers_inside::layout::MEMORY_BASE))
                    .unwrap(),
                wcontainers_inside::layout::MEMORY_BASE,
                MEMORY,
            )
            .expect("mapping memory");
        let loaded = load(&memory, program, CPUS).expect("loading the inside");
        let interrupts = machine.interrupts().expect("interrupt numbers");
        let start = Start {
            memory: MEMORY,
            host_page: page_size(),
            timer_interrupt: interrupts.timer.into(),
            first_raised_interrupt: interrupts.first_raised.into(),
            ..Start::default()
        };
        Arc::new(Shared {
            machine,
            memory,
            loaded,
            start,
            stop_settings,
            reports: Mutex::default(),
            measurements: Mutex::default(),
            raised_at: Mutex::default(),
            stop_delays: Mutex::default(),
            fatal: AtomicBool::new(false),
        })
    }

    fn page_size() -> u64 {
        // SAFETY: a plain query.
        unsafe { libc::sysconf(libc::_SC_PAGESIZE) as u64 }
    }

    /// Runs virtual CPU `index` on this thread until its inside is done.
    fn serve(shared: &Arc<Shared>, index: u64) {
        let mut cpu = shared.machine.cpu().expect("making a virtual CPU");
        boot(
            &mut cpu,
            &shared.memory,
            &shared.loaded,
            index,
            shared.start,
        )
        .expect("booting");
        let offset = cpu.counter_offset().expect("counter offset");
        let mut stops: Option<StopTest> = None;
        loop {
            let exit = match cpu.run() {
                Ok(exit) => exit,
                Err(e) => {
                    println!("cpu {index}: {e:?}");
                    shared.fatal.store(true, Ordering::Relaxed);
                    return;
                }
            };
            match exit {
                Exit::Doorbell => {
                    if !answer(shared, &mut cpu, index, offset, &mut stops) {
                        return;
                    }
                }
                Exit::Stopped => {
                    let at = now();
                    if let Some(test) = stops.as_mut() {
                        test.stopped(&cpu, at, shared);
                    }
                }
                Exit::Exception {
                    syndrome,
                    address,
                    physical,
                } => {
                    let pc = cpu.get(Register::Pc).unwrap_or_default();
                    println!(
                        "cpu {index}: exception the inside did not take: syndrome {syndrome:#x} address {address:#x} physical {physical:#x} pc {pc:#x}"
                    );
                    shared.fatal.store(true, Ordering::Relaxed);
                    return;
                }
            }
        }
    }

    /// Answers one doorbell; false when the inside is done.
    fn answer(
        shared: &Arc<Shared>,
        cpu: &mut impl VirtualCpu<Stopper = impl Stopper>,
        index: u64,
        offset: u64,
        stops: &mut Option<StopTest>,
    ) -> bool {
        let page = link_page(&shared.memory, &shared.loaded, index);
        let text = String::from_utf8_lossy(page.data()).into_owned();
        if let Some(request) = link::Request::from_word(page.request) {
            match request {
                link::Request::Print => println!("cpu {index}: {text}"),
                link::Request::Fatal => {
                    let Fatal { esr, elr, far } = page.get();
                    println!(
                        "cpu {index}: the inside faulted: ESR {esr:#x} ELR {elr:#x} FAR {far:#x}"
                    );
                    shared.fatal.store(true, Ordering::Relaxed);
                    return false;
                }
                link::Request::GiveBack => {
                    let GiveBack { at, length } = page.get();
                    shared
                        .machine
                        .give_back(&shared.memory, at, length)
                        .expect("giving back");
                }
            }
            return true;
        }
        let Some(request) = protocol::Request::from_word(page.request) else {
            println!("cpu {index}: unknown request {:#x}", page.request);
            shared.fatal.store(true, Ordering::Relaxed);
            return false;
        };
        match request {
            protocol::Request::Nothing | protocol::Request::Echo => {}
            protocol::Request::Done => return false,
            protocol::Request::Write => {
                let Write { at } = page.get();
                let bytes = page.data();
                // SAFETY: the inside gave this frame for the write and runs nothing from it until answered.
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        bytes.as_ptr(),
                        shared.memory.host(at),
                        bytes.len(),
                    )
                };
            }
            protocol::Request::Report => {
                let Report { case, passed } = page.get();
                let passed = passed == u64::from(true);
                shared
                    .reports
                    .lock()
                    .unwrap()
                    .push((Case::from_word(case), passed, text));
            }
            protocol::Request::Measurement => {
                let Measurement { case } = page.get();
                shared
                    .measurements
                    .lock()
                    .unwrap()
                    .push((Case::from_word(case), text));
            }
            protocol::Request::Footprint => page.put(&Bytes { bytes: footprint() }),
            protocol::Request::RaiseLater => {
                let RaiseLater {
                    interrupt,
                    delay_us,
                } = page.get();
                let shared = Arc::clone(shared);
                thread::spawn(move || {
                    thread::sleep(Duration::from_micros(delay_us));
                    shared.raised_at.lock().unwrap().insert(interrupt, now());
                    shared
                        .machine
                        .raise(interrupt as u32, true)
                        .expect("raising");
                });
            }
            protocol::Request::Lower => {
                let Lower { interrupt } = page.get();
                shared
                    .machine
                    .raise(interrupt as u32, false)
                    .expect("lowering");
                let raised = shared
                    .raised_at
                    .lock()
                    .unwrap()
                    .remove(&interrupt)
                    .unwrap_or_default();
                page.put(&CounterValue {
                    counter: raised.wrapping_sub(offset),
                });
            }
            protocol::Request::StartCpu => {
                let shared = Arc::clone(shared);
                let StartCpu { cpu: other } = page.get();
                thread::spawn(move || serve(&shared, other))
                    .join()
                    .expect("second virtual CPU");
            }
            protocol::Request::StopTest => {
                *stops = Some(StopTest::start(shared, cpu.stopper(), page.get()));
            }
        }
        true
    }

    /// Stops a running virtual CPU from another thread, as the cases' stop test asks.
    struct StopTest {
        asked: Arc<AtomicU64>,
        seen: Arc<AtomicU64>,
        stopped_at: Arc<AtomicU64>,
        loop_range: std::ops::Range<u64>,
        counter: u8,
        settings: StopSettings,
        outside_loop: u64,
        went_back: u64,
        last_count: u64,
    }

    impl StopTest {
        fn start(shared: &Arc<Shared>, stopper: impl Stopper, request: StopRequest) -> StopTest {
            let StopRequest {
                interrupt,
                loop_start,
                loop_end,
                counter,
            } = request;
            let settings = shared.stop_settings;
            let asked = Arc::new(AtomicU64::new(0));
            let seen = Arc::new(AtomicU64::new(0));
            let stopped_at = Arc::new(AtomicU64::new(0));
            let (a, s, t, shared) = (
                Arc::clone(&asked),
                Arc::clone(&seen),
                Arc::clone(&stopped_at),
                Arc::clone(shared),
            );
            thread::spawn(move || {
                let mut random = fastrand::Rng::with_seed(settings.seed);
                for stop in 1..=settings.stops {
                    let wait = random.u64(STOP_WAIT_US);
                    thread::sleep(Duration::from_micros(wait));
                    let decided = now();
                    a.store(stop, Ordering::SeqCst);
                    stopper.stop().expect("stopping");
                    while s.load(Ordering::SeqCst) != stop {
                        std::hint::spin_loop();
                    }
                    let delay = ticks_to_nanos(t.load(Ordering::SeqCst).saturating_sub(decided));
                    shared.stop_delays.lock().unwrap().push(delay);
                }
                shared.raised_at.lock().unwrap().insert(interrupt, now());
                shared
                    .machine
                    .raise(interrupt as u32, true)
                    .expect("raising");
            });
            StopTest {
                asked,
                seen,
                stopped_at,
                loop_range: loop_start..loop_end,
                counter: u8::try_from(counter).expect("a general register"),
                settings,
                outside_loop: 0,
                went_back: 0,
                last_count: 0,
            }
        }

        fn stopped(&mut self, cpu: &impl VirtualCpu, at: u64, shared: &Shared) {
            let asked = self.asked.load(Ordering::SeqCst);
            if asked == self.seen.load(Ordering::SeqCst) {
                return;
            }
            let pc = cpu.get(Register::Pc).unwrap_or_default();
            let count = cpu.get(Register::X(self.counter)).unwrap_or_default();
            self.outside_loop += u64::from(!self.loop_range.contains(&pc));
            self.went_back += u64::from(count < self.last_count);
            self.last_count = count;
            self.stopped_at.store(at, Ordering::SeqCst);
            self.seen.store(asked, Ordering::SeqCst);
            if asked == self.settings.stops {
                let passed = self.outside_loop == 0 && self.went_back == 0;
                shared.reports.lock().unwrap().push((
                    Some(Case::HostStops),
                    passed,
                    format!(
                        "{} stops from another host thread (seed {:#x}): pc outside the loop {} times, count went back {} times",
                        self.settings.stops, self.settings.seed, self.outside_loop, self.went_back
                    ),
                ));
            }
        }
    }

    fn print_delays(delays: &[f64], settings: StopSettings) {
        if delays.is_empty() {
            return;
        }
        let mut sorted = delays.to_vec();
        sorted.sort_by(f64::total_cmp);
        let at = |q: f64| sorted[((sorted.len() - 1) as f64 * q) as usize];
        println!(
            "stop delay from the decision to the virtual CPU's exit, {} stops, seed {:#x}, nanoseconds: min {:.0} median {:.0} 90th {:.0} 99th {:.0} max {:.0}",
            sorted.len(),
            settings.seed,
            sorted[0],
            at(0.5),
            at(0.9),
            at(0.99),
            sorted[sorted.len() - 1]
        );
    }

    fn now() -> u64 {
        // SAFETY: a plain clock read.
        unsafe { libc::mach_absolute_time() }
    }

    fn ticks_to_nanos(ticks: u64) -> f64 {
        let mut info = libc::mach_timebase_info { numer: 0, denom: 0 };
        // SAFETY: fills the struct.
        unsafe { libc::mach_timebase_info(&mut info) };
        ticks as f64 * f64::from(info.numer) / f64::from(info.denom)
    }

    fn footprint() -> u64 {
        // SAFETY: a zeroed plain struct the call fills.
        let mut info: libc::rusage_info_v4 = unsafe { std::mem::zeroed() };
        // SAFETY: the buffer is the size the flavour names.
        let done = unsafe {
            libc::proc_pid_rusage(libc::getpid(), libc::RUSAGE_INFO_V4, (&raw mut info).cast())
        };
        assert_eq!(done, 0, "proc_pid_rusage");
        info.ri_phys_footprint
    }

    /// The load averages the system keeps: over 1, 5 and 15 minutes.
    const LOAD_AVERAGES: usize = 3;

    fn load_average() -> String {
        let mut loads = [0f64; LOAD_AVERAGES];
        // SAFETY: fills three values.
        unsafe { libc::getloadavg(loads.as_mut_ptr(), loads.len() as i32) };
        format!("{:.1} {:.1} {:.1}", loads[0], loads[1], loads[2])
    }
}
