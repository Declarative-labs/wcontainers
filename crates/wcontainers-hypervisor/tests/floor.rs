//! N1's bar: boots the floor's cases program under the monitor, serves its link, makes the stops from another
//! thread, and checks every case passed. Then boots it once more in a machine with EL2 for the counter trial,
//! which is recorded only. Run signed with the hypervisor entitlement (see docs/status-native.md).

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
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::{Duration, Instant};

    use aarch64_cpu::registers::{HCR_EL2, ID_AA64DFR0_EL1};
    use wcontainers_hypervisor::hvf::{HvfMachine, HypervisorFramework};
    use wcontainers_hypervisor::{
        Exit, Hypervisor, Loaded, Machine, Memory, Register, Stopper, VirtualCpu, boot, link, load,
        programs,
    };
    use wcontainers_inside::link::{Request, Start};

    /// Bytes of guest-physical memory the test reserves.
    const MEMORY: u64 = 1 << 30;

    /// Virtual CPUs the cases use.
    const CPUS: u64 = 2;

    /// PMUVer of the first version of the performance monitors (PMUv3).
    const PMU_VERSION_3: u64 = 1;

    /// The stop test waits between stops for a seeded time in this range of microseconds.
    const STOP_WAIT_US: std::ops::Range<u64> = 200..2000;

    /// The stop test's seed.
    const STOP_SEED: u64 = 0x9e37_79b9_7f4a_7c15;

    /// One machine and what the cases told it.
    struct Shared {
        machine: HvfMachine,
        memory: Memory,
        loaded: Loaded,
        start: Start,
        reports: Mutex<Vec<(bool, String)>>,
        raised_at: Mutex<HashMap<u64, u64>>,
        stop_delays: Mutex<Vec<f64>>,
        fatal: AtomicBool,
        /// Reports the inside said it sent, when it finished.
        sent: AtomicU64,
    }

    pub fn main() {
        let el2_supported =
            applevisor::prelude::VirtualMachineConfig::get_el2_supported().unwrap_or(false);
        let started = Instant::now();
        let shared = machine(false);
        serve(&shared, 0);
        let reports = shared.reports.lock().unwrap().clone();
        let delays = shared.stop_delays.lock().unwrap().clone();
        let fatal = shared.fatal.load(Ordering::Relaxed);
        let sent = shared.sent.load(Ordering::Relaxed);
        drop(shared);
        let mut failed = 0;
        for (passed, text) in &reports {
            println!("{} {text}", if *passed { "pass" } else { "FAIL" });
            failed += usize::from(!*passed);
        }
        print_delays(&delays);
        println!("load {}", load_average());
        println!(
            "{} cases, {failed} failed, in {:?}",
            reports.len(),
            started.elapsed()
        );
        if el2_supported {
            let trial = machine(true);
            serve(&trial, 0);
            for (_, text) in trial.reports.lock().unwrap().iter() {
                println!("recorded {text}");
            }
            if trial.fatal.load(Ordering::Relaxed) {
                println!("recorded: the counter trial's inside faulted (see above)");
            }
        } else {
            println!(
                "recorded: this host gives no EL2 to virtual machines, so the counter trial did not run"
            );
        }
        let expected_stops = usize::try_from(STOPS).unwrap();
        assert!(!fatal, "the inside faulted");
        assert_eq!(failed, 0, "cases failed");
        assert_eq!(reports.len() as u64, sent + 1, "a report went missing");
        assert_eq!(delays.len(), expected_stops, "not every stop was seen");
    }

    /// Stops the cases program asks for.
    const STOPS: u64 = 300;

    fn machine(el2: bool) -> Arc<Shared> {
        let machine = HypervisorFramework::machine(el2).expect("making the machine");
        let memory = Memory::reserve(MEMORY).expect("reserving memory");
        machine
            .map(
                std::ptr::NonNull::new(memory.host(wcontainers_inside::layout::MEMORY_BASE))
                    .unwrap(),
                wcontainers_inside::layout::MEMORY_BASE,
                MEMORY,
            )
            .expect("mapping memory");
        let loaded = load(&memory, programs::FLOOR_CASES, CPUS).expect("loading the inside");
        let interrupts = machine.interrupts().expect("interrupt numbers");
        let start = Start {
            memory: MEMORY,
            cpus: CPUS,
            host_page: page_size(),
            timer_interrupt: interrupts.timer.into(),
            counter_interrupt: interrupts.counter.into(),
            first_raised_interrupt: interrupts.first_raised.into(),
            el2: el2.into(),
            ..Start::default()
        };
        Arc::new(Shared {
            machine,
            memory,
            loaded,
            start,
            reports: Mutex::default(),
            raised_at: Mutex::default(),
            stop_delays: Mutex::default(),
            fatal: AtomicBool::new(false),
            sent: AtomicU64::new(0),
        })
    }

    fn page_size() -> u64 {
        // SAFETY: a plain query.
        unsafe { libc::sysconf(libc::_SC_PAGESIZE) as u64 }
    }

    /// Runs virtual CPU `index` on this thread until its inside is done.
    fn serve(shared: &Arc<Shared>, index: u64) {
        let mut cpu = shared.machine.cpu().expect("making a virtual CPU");
        if shared.start.el2 != 0 {
            let debug = cpu.get(Register::DebugFeatures).unwrap();
            let field = ID_AA64DFR0_EL1::PMUVer;
            let debug = (debug & !(field.mask << field.shift)) | field.val(PMU_VERSION_3).value;
            let set = cpu.set(Register::DebugFeatures, debug).and_then(|()| {
                cpu.set(Register::HypervisorControl, HCR_EL2::RW::EL1IsAarch64.value)
            });
            let read_back = cpu.get(Register::DebugFeatures);
            shared.reports.lock().unwrap().push((true, format!("counter trial: ID_AA64DFR0_EL1 set to {debug:#x}: {set:?}, reads {read_back:x?}")));
        }
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
        let page = link(&shared.memory, &shared.loaded, index);
        let text = String::from_utf8_lossy(&page.bytes[..page.length as usize]).into_owned();
        let words = page.words;
        match Request::from_word(page.request) {
            Some(Request::Nothing) => {}
            Some(Request::Print) => println!("cpu {index}: {text}"),
            Some(Request::Done) => {
                // Every virtual CPU counts its reports in the same image; the first finishes last.
                if index == 0 {
                    shared.sent.store(words[0], Ordering::Relaxed);
                }
                return false;
            }
            Some(Request::Fatal) => {
                println!(
                    "cpu {index}: the inside faulted: ESR {:#x} ELR {:#x} FAR {:#x}",
                    words[0], words[1], words[2]
                );
                shared.fatal.store(true, Ordering::Relaxed);
                return false;
            }
            Some(Request::Write) => {
                let bytes = &page.bytes[..page.length as usize];
                // SAFETY: the inside gave this frame for the write and runs nothing from it until answered.
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        bytes.as_ptr(),
                        shared.memory.host(words[0]),
                        bytes.len(),
                    )
                };
            }
            Some(Request::GiveBack) => shared
                .memory
                .give_back(&shared.machine, words[0], words[1])
                .expect("giving back"),
            Some(Request::Report) => shared.reports.lock().unwrap().push((words[0] == 1, text)),
            Some(Request::Footprint) => page.words[0] = footprint(),
            Some(Request::RaiseLater) => {
                let (interrupt, delay) = (words[0], words[1]);
                let shared = Arc::clone(shared);
                thread::spawn(move || {
                    thread::sleep(Duration::from_micros(delay));
                    shared.raised_at.lock().unwrap().insert(interrupt, now());
                    shared
                        .machine
                        .raise(interrupt as u32, true)
                        .expect("raising");
                });
            }
            Some(Request::Lower) => {
                shared
                    .machine
                    .raise(words[0] as u32, false)
                    .expect("lowering");
                let raised = shared
                    .raised_at
                    .lock()
                    .unwrap()
                    .remove(&words[0])
                    .unwrap_or_default();
                page.words[0] = raised.wrapping_sub(offset);
            }
            Some(Request::StartCpu) => {
                let shared = Arc::clone(shared);
                let other = words[0];
                thread::spawn(move || serve(&shared, other))
                    .join()
                    .expect("second virtual CPU");
            }
            Some(Request::StopTest) => {
                let test = StopTest::start(
                    shared,
                    cpu.stopper(),
                    words[0],
                    words[1],
                    words[2]..words[3],
                    words[4] as u8,
                );
                *stops = Some(test);
            }
            None => {
                println!("cpu {index}: unknown request {:#x}", page.request);
                shared.fatal.store(true, Ordering::Relaxed);
                return false;
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
        outside_loop: u64,
        went_back: u64,
        last_count: u64,
    }

    impl StopTest {
        fn start(
            shared: &Arc<Shared>,
            stopper: impl Stopper,
            count: u64,
            interrupt: u64,
            loop_range: std::ops::Range<u64>,
            counter: u8,
        ) -> StopTest {
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
                let mut random = fastrand::Rng::with_seed(STOP_SEED);
                for stop in 1..=count {
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
                loop_range,
                counter,
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
            if asked == STOPS {
                let passed = self.outside_loop == 0 && self.went_back == 0;
                shared.reports.lock().unwrap().push((
                    passed,
                    format!("{STOPS} stops from another host thread: pc outside the loop {} times, count went back {} times", self.outside_loop, self.went_back),
                ));
            }
        }
    }

    fn print_delays(delays: &[f64]) {
        if delays.is_empty() {
            return;
        }
        let mut sorted = delays.to_vec();
        sorted.sort_by(f64::total_cmp);
        let at = |q: f64| sorted[((sorted.len() - 1) as f64 * q) as usize];
        println!(
            "stop delay from the decision to the virtual CPU's exit, {} stops, nanoseconds: min {:.0} median {:.0} 90th {:.0} 99th {:.0} max {:.0}",
            sorted.len(),
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
