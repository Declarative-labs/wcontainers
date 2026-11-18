//! The cases. Each prints one report: its name, whether it passed, and what it saw.

use core::arch::{asm, global_asm};
use core::fmt::Write as _;
use core::sync::atomic::{AtomicU64, Ordering};

use aarch64_cpu::registers::{ESR_EL1, FAR_EL1, MDSCR_EL1, Read, SPSR_EL1, TPIDRRO_EL0, Write};
use arm_sysregs::el0::registers::PmcrEl0;
use wcontainers_abi::PAGE_SIZE;
use wcontainers_inside::floor::{
    self, AddressSpace, Floor, Taken, UserRegisters, code, counter, counter_frequency, enter_user,
    guest_read_only, guest_read_write, heap, make_code_visible, process_code, register_page,
    set_stop_word, with_frames, write_frame,
};
use wcontainers_inside::layout::{
    BLOCK_LEVEL, BLOCK_SIZE, LEAF_LEVEL, MIRROR, Region, level_span, linear,
};
use wcontainers_inside::link::{Request, Start};

/// Marks the cases put in registers and memory to tell their pages apart.
const MARK_A: u64 = 0xa1;
const MARK_B: u64 = 0xb2;

/// A tag in a pointer's top byte.
const TAG: u64 = 0x5a;

/// Where a pointer's tag sits: its top byte.
const TAG_SHIFT: u32 = u64::BITS - u8::BITS;

/// Guest addresses the cases map their data at.
const GUEST: u64 = 0x10_0000;

/// Times the cases that switch views go back and forth.
const ROUNDS: u64 = 24;

/// Doorbell round trips timed.
const DOORBELLS: u64 = 1000;

/// Interrupt numbers the host raises, from the first one it may.
const RAISED: u32 = 1;

/// Microseconds the host waits before raising an interrupt.
const RAISE_DELAY_US: u64 = 200;

/// Counter ticks before a timer case's interrupt.
const TIMER_TICKS: u64 = 2400;

/// Stops the host makes from another thread.
const STOPS: u64 = 300;

/// Blocks of memory the give-back case touches and gives back.
const GIVE_BACK_BLOCKS: u64 = 32;

/// Instructions the software step case steps through before its system call.
const STEPS: u64 = 8;

/// Nanoseconds in a second.
const NANOS: u64 = 1_000_000_000;

/// The data or instruction fault status code in ESR_EL1's ISS.
const FAULT_STATUS: u64 = 0b11_1111;

/// The permission fault status code, plus the level the fault happened at.
const PERMISSION_FAULT: u64 = 0b00_1100;

/// ESR_EL1's ISS bit that says a data abort came from a write.
const WRITE_NOT_READ: u64 = 1 << 6;

/// The architecture's common event for instructions retired.
const INSTRUCTIONS_RETIRED: u64 = 0x08;

/// PMEVTYPER<n>_EL0.P: the counter does not count at EL1.
const EXCLUDE_EL1: u64 = 1 << 31;

/// Counter 0's bit in the counter enable and interrupt enable registers.
const COUNTER_0: u64 = 1;

/// PMCNTENSET_EL0.C and PMINTENSET_EL1.C: the cycle counter's bit.
const CYCLE_COUNTER: u64 = 1 << 31;

/// EL0 cycles the cycle counter trial lets pass before the overflow.
const TRIAL_CYCLES: u64 = 1_000_000;

/// EL0 instructions the counter trial lets pass before the overflow.
const TRIAL_INSTRUCTIONS: u64 = 1_000_000;

/// Counter ticks the counter trial waits for the overflow before giving up.
const TRIAL_TIMEOUT_TICKS: u64 = 24_000_000;

/// The second virtual CPU's address-space identifiers start here, apart from the first's.
const SECOND_CPU_ASIDS: u16 = 0x100;

/// The register the spin snippet counts its rounds in.
const SPIN_COUNTER: usize = 19;

/// Instructions in one round of the spin snippet.
const SPIN_INSTRUCTIONS: u64 = 2;

macro_rules! snippets {
    ($($name:ident: $body:literal;)*) => {
        mod starts {
            unsafe extern "C" {
                $(
                    #[link_name = concat!("snippet_", stringify!($name))]
                    pub static $name: u8;
                )*
            }
        }
        global_asm!(
            ".section .rodata.snippets, \"a\"",
            ".balign {instruction}",
            $(
                concat!(".global snippet_", stringify!($name)),
                concat!("snippet_", stringify!($name), ":"),
                $body,
                concat!(".global snippet_", stringify!($name), "_end"),
                concat!("snippet_", stringify!($name), "_end:"),
            )*
            mark_a = const MARK_A,
            mark_b = const MARK_B,
            instruction = const size_of::<u32>(),
            counter = const SPIN_COUNTER,
        );
        mod ends {
            unsafe extern "C" {
                $(
                    #[link_name = concat!("snippet_", stringify!($name), "_end")]
                    pub static $name: u8;
                )*
            }
        }
        /// Snippets of guest-level code, by name: their bytes.
        mod code_of {
            $(
                pub fn $name() -> &'static [u8] {
                    let start = &raw const super::starts::$name;
                    let end = &raw const super::ends::$name;
                    // SAFETY: the snippet's bytes lie between its two labels in the image.
                    unsafe { core::slice::from_raw_parts(start, end as usize - start as usize) }
                }
            )*
        }
    };
}

snippets! {
    mark_a: "movz x0, #{mark_a}\n svc #0";
    mark_b: "movz x0, #{mark_b}\n svc #0";
    load: "ldr x0, [x1]\n svc #0";
    store: "str x2, [x1]\n svc #0";
    branch: "br x1";
    own_page: "adr x1, .\n ldr x0, [x1]\n svc #0";
    spin: "0: add x{counter}, x{counter}, #1\n b 0b";
    read_stop_word: "mrs x0, tpidrro_el0\n svc #0";
    write_stop_word: "msr tpidrro_el0, x1\n svc #0";
    steps: "add x0, x0, #1\n add x0, x0, #1\n add x0, x0, #1\n add x0, x0, #1\n add x0, x0, #1\n add x0, x0, #1\n add x0, x0, #1\n add x0, x0, #1\n svc #0";
}

/// What one run at the guest's level gave back.
struct Run {
    taken: Taken,
    class: u64,
    status: u64,
    write: bool,
    far: u64,
    registers: UserRegisters,
}

/// Runs the guest's level in `space` from `pc` with x1 and x2 set.
fn run(space: &AddressSpace, pc: u64, x1: u64, x2: u64) -> Run {
    let mut registers = UserRegisters {
        pc,
        pstate: SPSR_EL1::M::EL0t.value,
        ..UserRegisters::default()
    };
    registers.x[1] = x1;
    registers.x[2] = x2;
    resume(space, registers)
}

fn resume(space: &AddressSpace, mut registers: UserRegisters) -> Run {
    space.switch();
    let taken = enter_user(&mut registers);
    let esr = ESR_EL1.get();
    Run {
        taken,
        class: ESR_EL1.read(ESR_EL1::EC),
        status: esr & FAULT_STATUS,
        write: esr & WRITE_NOT_READ != 0,
        far: FAR_EL1.get(),
        registers,
    }
}

fn class(value: ESR_EL1::EC::Value) -> u64 {
    value as u64
}

/// A system call came back with x0 = `x0`.
fn called(run: &Run, x0: u64) -> bool {
    run.taken == Taken::Synchronous
        && run.class == class(ESR_EL1::EC::Value::SVC64)
        && run.registers.x[0] == x0
}

/// A data abort came back: a permission fault at the leaf level at `far`, from a write or not.
fn refused(run: &Run, far: u64, write: bool) -> bool {
    run.class == class(ESR_EL1::EC::Value::DataAbortLowerEL)
        && run.status == PERMISSION_FAULT + u64::from(LEAF_LEVEL)
        && run.far == far
        && run.write == write
}

/// Reports this virtual CPU has sent.
static REPORTS: AtomicU64 = AtomicU64::new(0);

/// Sends one case's report.
fn report(name: &str, passed: bool, details: core::fmt::Arguments) {
    REPORTS.fetch_add(1, Ordering::Relaxed);
    floor::send(
        Request::Report,
        &[u64::from(passed)],
        format_args!("{name}: {details}"),
    );
}

/// The cases' shared state.
struct Cases {
    floor: Floor,
    /// The next free page of the code area.
    next_code: u64,
    /// The next free guest address.
    next_guest: u64,
    /// The next address-space identifier.
    next_asid: u16,
}

impl Cases {
    /// A page of the code area holding `bytes`, made visible.
    fn code_page(&mut self, bytes: &[u8]) -> u64 {
        let va = self.next_code;
        self.next_code += PAGE_SIZE;
        let global = self.floor.global;
        with_frames(|frames| {
            let frame = frames.take();
            write_frame(frame, 0, bytes);
            global.map(frames, va, frame, code());
        });
        make_code_visible(va, bytes.len() as u64);
        va
    }

    fn guest_page(&mut self) -> u64 {
        let va = self.next_guest;
        self.next_guest += PAGE_SIZE;
        va
    }

    fn process(&mut self) -> AddressSpace {
        self.next_asid += 1;
        let (global, asid) = (self.floor.global, self.next_asid);
        with_frames(|frames| AddressSpace::new(frames, &global, asid))
    }

    fn execute_only(&mut self) {
        let target = self.code_page(code_of::mark_a());
        let load = self.code_page(code_of::load());
        let store = self.code_page(code_of::store());
        let own = self.code_page(code_of::own_page());
        let space = self.process();
        let ran = run(&space, target, 0, 0);
        let loaded = run(&space, load, target, 0);
        let stored = run(&space, store, target, MARK_B);
        let own_load = run(&space, own, 0, 0);
        let passed = called(&ran, MARK_A)
            && refused(&loaded, target, false)
            && refused(&stored, target, true)
            && refused(&own_load, own, false);
        report(
            "execute-only code runs and refuses loads and stores",
            passed,
            format_args!(
                "run x0 {:#x}; load EC {:#x} FSC {:#x} FAR {:#x}; store EC {:#x} FSC {:#x} write {} FAR {:#x}; own page FAR {:#x}",
                ran.registers.x[0],
                loaded.class,
                loaded.status,
                loaded.far,
                stored.class,
                stored.status,
                stored.write,
                stored.far,
                own_load.far
            ),
        );
    }

    fn guest_no_execute(&mut self) {
        let space = self.process();
        let page = self.guest_page();
        with_frames(|frames| {
            let frame = frames.take();
            write_frame(frame, 0, code_of::mark_a());
            space.user.map(frames, page, frame, guest_read_write());
        });
        let ran = run(&space, page, 0, 0);
        let passed = ran.class == class(ESR_EL1::EC::Value::InstrAbortLowerEL)
            && ran.status == PERMISSION_FAULT + u64::from(LEAF_LEVEL)
            && ran.far == page
            && ran.registers.pc == page;
        report(
            "guest pages refuse execution",
            passed,
            format_args!(
                "EC {:#x} FSC {:#x} FAR {:#x} ELR {:#x}",
                ran.class, ran.status, ran.far, ran.registers.pc
            ),
        );
    }

    fn misaligned_branch(&mut self) {
        let target = self.code_page(code_of::mark_a());
        let branch = self.code_page(code_of::branch());
        let space = self.process();
        let mut passed = true;
        let mut out = Line::new();
        for offset in [1, 2, 3] {
            let ran = run(&space, branch, target + offset, 0);
            passed &= ran.class == class(ESR_EL1::EC::Value::PCAlignmentFault)
                && ran.far == target + offset
                && ran.registers.pc == target + offset;
            let _ = write!(
                out,
                "offset {offset}: EC {:#x} FAR {:#x} ELR {:#x}; ",
                ran.class, ran.far, ran.registers.pc
            );
        }
        report(
            "a misaligned br faults at its target",
            passed,
            format_args!("{}", out.as_str()),
        );
    }

    fn tagged(&mut self) {
        let load = self.code_page(code_of::load());
        let branch = self.code_page(code_of::branch());
        let space = self.process();
        let page = self.guest_page();
        with_frames(|frames| {
            let frame = frames.take();
            write_frame(frame, 0, &MARK_A.to_le_bytes());
            space.user.map(frames, page, frame, guest_read_write());
        });
        let tagged = page | (TAG << TAG_SHIFT);
        let loaded = run(&space, load, tagged, 0);
        let branched = run(&space, branch, tagged, 0);
        let passed = called(&loaded, MARK_A)
            && branched.class == class(ESR_EL1::EC::Value::InstrAbortLowerEL)
            && branched.status == PERMISSION_FAULT + u64::from(LEAF_LEVEL)
            && branched.far == page;
        report(
            "tagged loads and branches ignore the top byte (TBI0)",
            passed,
            format_args!(
                "load x0 {:#x}; branch to {tagged:#x}: EC {:#x} FSC {:#x} FAR {:#x}",
                loaded.registers.x[0], branched.class, branched.status, branched.far
            ),
        );
    }

    fn stop_word(&mut self) {
        let read = self.code_page(code_of::read_stop_word());
        let write = self.code_page(code_of::write_stop_word());
        let space = self.process();
        set_stop_word(MARK_B);
        let ran = run(&space, read, 0, 0);
        let wrote = run(&space, write, MARK_A, 0);
        let after = TPIDRRO_EL0.get();
        let passed = called(&ran, MARK_B)
            && wrote.class == class(ESR_EL1::EC::Value::Unknown)
            && wrote.registers.pc == write
            && after == MARK_B;
        report(
            "TPIDRRO_EL0 is written at EL1, read at EL0, and an EL0 write is undefined",
            passed,
            format_args!(
                "read {:#x}; write EC {:#x} at {:#x}; after {after:#x}",
                ran.registers.x[0], wrote.class, wrote.registers.pc
            ),
        );
    }

    /// Maps one page in `space`'s inside half at `va` holding `mark`.
    fn inside_page(
        &self,
        space: &AddressSpace,
        va: u64,
        mark: u64,
        attributes: aarch64_paging::descriptor::El1Attributes,
    ) -> u64 {
        with_frames(|frames| {
            let frame = frames.take();
            write_frame(frame, 0, &mark.to_le_bytes());
            space.inside.map(frames, va, frame, attributes);
            frame
        })
    }

    fn register_page(&mut self) {
        let load = self.code_page(code_of::load());
        let store = self.code_page(code_of::store());
        let space = self.process();
        let va = Region::Threads.base();
        let frame = self.inside_page(&space, va, MARK_A, register_page());
        let loaded = run(&space, load, va, 0);
        let stored = run(&space, store, va, MARK_B);
        // SAFETY: the frame is the register page's, read through the linear map.
        let after = unsafe { core::ptr::read_volatile(linear(frame) as *const u64) };
        let passed = called(&loaded, MARK_A)
            && stored.class == class(ESR_EL1::EC::Value::SVC64)
            && after == MARK_B;
        report(
            "the register page is readable and writable at EL0",
            passed,
            format_args!(
                "load {:#x}; store EC {:#x}; after {after:#x}",
                loaded.registers.x[0], stored.class
            ),
        );
    }

    fn two_containers(&mut self) {
        let branch = self.code_page(code_of::branch());
        let load = self.code_page(code_of::load());
        let mut sides = [
            (self.process(), MARK_A, code_of::mark_a()),
            (self.process(), MARK_B, code_of::mark_b()),
        ];
        for (space, mark, mirror_code) in sides.iter_mut() {
            with_frames(|frames| {
                let frame = frames.take();
                write_frame(frame, 0, mirror_code);
                space.inside.map(frames, MIRROR, frame, process_code());
            });
            make_code_visible_in(space, MIRROR, mirror_code.len() as u64);
            self.inside_page(space, Region::Threads.base(), *mark, register_page());
            self.inside_page(space, Region::Heap.base(), *mark, heap());
        }
        let mut passed = true;
        let mut out = Line::new();
        for round in 0..ROUNDS {
            for (space, mark, _) in &sides {
                let mirror = run(space, branch, MIRROR, 0);
                let registers = run(space, load, Region::Threads.base(), 0);
                // SAFETY: the heap page is mapped in this space's inside half, now switched to.
                let heap = unsafe { core::ptr::read_volatile(Region::Heap.base() as *const u64) };
                let ok = called(&mirror, *mark) && called(&registers, *mark) && heap == *mark;
                if !ok && passed {
                    let _ = write!(
                        out,
                        "round {round} asid {}: mirror {:#x} registers {:#x} heap {heap:#x}",
                        space.asid, mirror.registers.x[0], registers.registers.x[0]
                    );
                }
                passed &= ok;
            }
        }
        report(
            "two containers' processes see only their own mirror, register page and heap",
            passed,
            format_args!(
                "{ROUNDS} rounds of switching with no invalidation {}",
                out.as_str()
            ),
        );
    }

    fn thread_views(&mut self) {
        let store = self.code_page(code_of::store());
        let load = self.code_page(code_of::load());
        let process = self.process();
        let page = self.guest_page();
        self.next_asid += 1;
        let writer_asid = self.next_asid;
        self.next_asid += 1;
        let reader_asid = self.next_asid;
        let (writer, reader) = with_frames(|frames| {
            let frame = frames.take();
            let writer = process.view(frames, writer_asid);
            let reader = process.view(frames, reader_asid);
            writer.user.map(frames, page, frame, guest_read_write());
            reader.user.map(frames, page, frame, guest_read_only());
            (writer, reader)
        });
        let mut passed = true;
        for round in 0..ROUNDS {
            let wrote = run(&writer, store, page, round);
            let refused_write = run(&reader, store, page, MARK_B);
            let read = run(&reader, load, page, 0);
            passed &=
                called(&wrote, 0) && refused(&refused_write, page, true) && called(&read, round);
        }
        report(
            "two threads of one process with their own table roots see one page writable and read-only",
            passed,
            format_args!(
                "{ROUNDS} rounds, identifiers {writer_asid} and {reader_asid}, no invalidation"
            ),
        );
    }

    fn adjacent_pages(&mut self) {
        let store = self.code_page(code_of::store());
        let load = self.code_page(code_of::load());
        let space = self.process();
        let first = self.guest_page();
        let second = self.guest_page();
        let host_page = self.floor.start.host_page;
        let frame = with_frames(|frames| {
            let frame = frames.take_block();
            space.user.map(frames, first, frame, guest_read_write());
            space
                .user
                .map(frames, second, frame + PAGE_SIZE, guest_read_only());
            frame
        });
        let same_host_page = frame / host_page == (frame + PAGE_SIZE) / host_page;
        let wrote_first = run(&space, store, first, MARK_A);
        let wrote_second = run(&space, store, second, MARK_B);
        let read_second = run(&space, load, second, 0);
        let passed = same_host_page
            && called(&wrote_first, 0)
            && refused(&wrote_second, second, true)
            && called(&read_second, 0);
        report(
            "two adjacent guest pages in one host page keep their own permissions",
            passed,
            format_args!(
                "frames {frame:#x} and {:#x}, host page {host_page}, one host page {same_host_page}; second store EC {:#x} FSC {:#x} FAR {:#x}",
                frame + PAGE_SIZE,
                wrote_second.class,
                wrote_second.status,
                wrote_second.far
            ),
        );
    }

    fn block(&mut self) {
        let store = self.code_page(code_of::store());
        let load = self.code_page(code_of::load());
        let space = self.process();
        let base = self.next_guest.next_multiple_of(BLOCK_SIZE);
        self.next_guest = base + BLOCK_SIZE;
        with_frames(|frames| {
            let block = frames.take_block();
            space
                .user
                .map_block(frames, base, block, guest_read_write());
        });
        let pages = BLOCK_SIZE / PAGE_SIZE;
        let level_before = space.user.find(base).map(|(_, level)| level);
        let mut passed = level_before == Some(BLOCK_LEVEL);
        for page in 0..pages {
            passed &= called(&run(&space, store, base + page * PAGE_SIZE, page), 0);
        }
        let changed = base + (pages / 2) * PAGE_SIZE;
        with_frames(|frames| {
            space
                .user
                .protect(frames, changed, guest_read_only(), space.asid)
        });
        let level_after = space.user.find(base).map(|(_, level)| level);
        passed &= level_after == Some(LEAF_LEVEL);
        let refused_write = run(&space, store, changed, MARK_A);
        passed &= refused(&refused_write, changed, true);
        for page in 0..pages {
            let va = base + page * PAGE_SIZE;
            passed &= called(&run(&space, load, va, 0), page);
            if va != changed {
                passed &= called(&run(&space, store, va, page), 0);
            }
        }
        report(
            "a block entry reads and writes at EL0, and after one page's change only that page refuses",
            passed,
            format_args!(
                "level before {level_before:?} after {level_after:?}; changed page {changed:#x}: EC {:#x} FAR {:#x}",
                refused_write.class, refused_write.far
            ),
        );
    }

    fn late_code(&mut self) {
        let page = self.code_page(code_of::mark_a());
        let frame = self.floor.global.frame(page).expect("code page");
        let space = self.process();
        let half = PAGE_SIZE / 2;
        let mut passed = true;
        let mut wrong = 0;
        for round in 0..ROUNDS {
            let ran = run(&space, page, 0, 0);
            passed &= called(&ran, MARK_A);
            let (bytes, mark) = if round % 2 == 0 {
                (code_of::mark_b(), MARK_B)
            } else {
                (code_of::mark_a(), MARK_A)
            };
            floor::ask(Request::Write, &[frame + half], bytes);
            make_code_visible(page + half, bytes.len() as u64);
            let late = run(&space, page + half, 0, 0);
            if !called(&late, mark) {
                wrong += 1;
            }
        }
        passed &= wrong == 0;
        report(
            "code the monitor writes next to code that ran runs as written after maintenance",
            passed,
            format_args!("{ROUNDS} rounds, {wrong} ran stale"),
        );
    }

    fn step(&mut self) {
        let steps = self.code_page(code_of::steps());
        let space = self.process();
        MDSCR_EL1.modify(MDSCR_EL1::SS::SoftwareStepEnabled);
        let mut registers = UserRegisters {
            pc: steps,
            pstate: SPSR_EL1::M::EL0t.value,
            ..UserRegisters::default()
        };
        let mut passed = true;
        let mut out = Line::new();
        for step in 0..STEPS {
            let before = registers.pc;
            registers.pstate |= SPSR_EL1::SS::SET.value;
            let ran = resume(&space, registers);
            let ok = ran.class == class(ESR_EL1::EC::Value::SoftwareStepLowerEL)
                && ran.registers.pc == before + size_of::<u32>() as u64
                && ran.registers.x[0] == step + 1;
            if !ok && passed {
                let _ = write!(
                    out,
                    "step {step}: EC {:#x} ELR {:#x}",
                    ran.class, ran.registers.pc
                );
            }
            passed &= ok;
            registers = ran.registers;
        }
        registers.pstate |= SPSR_EL1::SS::SET.value;
        let last = resume(&space, registers);
        MDSCR_EL1.modify(MDSCR_EL1::SS::SoftwareStepDisabled);
        passed &= called(&last, STEPS);
        report(
            "EL1 software step of EL0 stops after each instruction",
            passed,
            format_args!(
                "{STEPS} steps, then the system call gives EC {:#x}; {}",
                last.class,
                out.as_str()
            ),
        );
    }

    fn timer(&mut self) {
        let spin = self.code_page(code_of::spin());
        let space = self.process();
        let mut late = [0u64; ROUNDS as usize];
        let mut passed = true;
        for slot in late.iter_mut() {
            let at = counter() + TIMER_TICKS;
            floor::arm_timer(at);
            let ran = run(&space, spin, 0, 0);
            let now = counter();
            let id = self.floor.take_interrupt();
            floor::disarm_timer();
            passed &= ran.taken == Taken::Interrupt
                && id == Some(self.floor.start.timer_interrupt as u32);
            *slot = ticks_to_nanos(now - at);
        }
        late.sort_unstable();
        report(
            "the virtual timer's interrupt lands in the inside",
            passed,
            format_args!(
                "{ROUNDS} interrupts, nanoseconds after the deadline: median {} max {}",
                late[late.len() / 2],
                late[late.len() - 1]
            ),
        );
    }

    fn raised(&mut self) {
        let spin = self.code_page(code_of::spin());
        let space = self.process();
        let interrupt = self.floor.start.first_raised_interrupt;
        let mut delay = [0u64; ROUNDS as usize];
        let mut passed = true;
        for slot in delay.iter_mut() {
            floor::ask(Request::RaiseLater, &[interrupt, RAISE_DELAY_US], &[]);
            let ran = run(&space, spin, 0, 0);
            let now = counter();
            let id = self.floor.take_interrupt();
            let raised_at = floor::ask(Request::Lower, &[interrupt], &[])[0];
            passed &=
                ran.taken == Taken::Interrupt && id == Some(interrupt as u32) && now >= raised_at;
            *slot = ticks_to_nanos(now.saturating_sub(raised_at));
        }
        delay.sort_unstable();
        report(
            "an interrupt the host raises from another thread lands in the inside",
            passed,
            format_args!(
                "{ROUNDS} interrupts, nanoseconds from raise to the inside: median {} max {}",
                delay[delay.len() / 2],
                delay[delay.len() - 1]
            ),
        );
    }

    fn doorbell(&mut self) {
        let start = counter();
        for _ in 0..DOORBELLS {
            floor::ask(Request::Nothing, &[], &[]);
        }
        let each = ticks_to_nanos(counter() - start) / DOORBELLS;
        report(
            "the doorbell's round trip",
            true,
            format_args!("{DOORBELLS} round trips, {each} ns each"),
        );
    }

    fn give_back(&mut self) {
        let length = GIVE_BACK_BLOCKS * BLOCK_SIZE;
        let first = with_frames(|frames| {
            let first = frames.take_block();
            for _ in 1..GIVE_BACK_BLOCKS {
                frames.take_block();
            }
            first
        });
        let mut at = first;
        while at < first + length {
            write_frame(at, 0, &MARK_A.to_le_bytes());
            at += PAGE_SIZE;
        }
        let before = floor::ask(Request::Footprint, &[], &[])[0];
        floor::ask(Request::GiveBack, &[first, length], &[]);
        let after = floor::ask(Request::Footprint, &[], &[])[0];
        // SAFETY: the given-back memory is still mapped, and reads as zeros.
        let reads = unsafe { core::ptr::read_volatile(linear(first + length / 2) as *const u64) };
        let fell = before.saturating_sub(after);
        let passed = fell >= length / 2 && reads == 0;
        report(
            "memory given back through the link lowers the process's footprint",
            passed,
            format_args!(
                "gave back {length} bytes; footprint {before} to {after}, fell {fell}; reads {reads:#x} after"
            ),
        );
    }

    fn second_cpu(&mut self) {
        floor::ask(Request::StartCpu, &[1], &[]);
    }

    fn stops(&mut self) {
        let spin = self.code_page(code_of::spin());
        let space = self.process();
        let interrupt = self.floor.start.first_raised_interrupt;
        let end = spin + code_of::spin().len() as u64;
        floor::ask(
            Request::StopTest,
            &[STOPS, interrupt, spin, end, SPIN_COUNTER as u64],
            &[],
        );
        let ran = run(&space, spin, 0, 0);
        let id = self.floor.take_interrupt();
        floor::ask(Request::Lower, &[interrupt], &[]);
        let passed = ran.taken == Taken::Interrupt && id == Some(interrupt as u32);
        report(
            "the spin stopped from another host thread ends by a raised interrupt",
            passed,
            format_args!("{} rounds of the spin", ran.registers.x[SPIN_COUNTER]),
        );
    }
}

/// Code written into `space`'s own pages made visible, with `space` switched to.
fn make_code_visible_in(space: &AddressSpace, va: u64, length: u64) {
    space.switch();
    make_code_visible(va, length);
}

fn ticks_to_nanos(ticks: u64) -> u64 {
    ticks * NANOS / counter_frequency()
}

/// A short line of text built on the stack.
struct Line {
    bytes: [u8; LINE],
    length: usize,
}

/// Bytes a [`Line`] holds.
const LINE: usize = 512;

impl Line {
    fn new() -> Line {
        Line {
            bytes: [0; LINE],
            length: 0,
        }
    }
    fn as_str(&self) -> &str {
        core::str::from_utf8(&self.bytes[..self.length]).unwrap_or("")
    }
}

impl core::fmt::Write for Line {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let take = s.len().min(LINE - self.length);
        self.bytes[self.length..self.length + take].copy_from_slice(&s.as_bytes()[..take]);
        self.length += take;
        Ok(())
    }
}

/// The second virtual CPU: its own process, a page of code, one run.
fn second(floor: Floor) {
    let mut cases = Cases {
        floor,
        next_code: Region::Code.base() + level_span(1),
        next_guest: GUEST,
        next_asid: SECOND_CPU_ASIDS,
    };
    let target = cases.code_page(code_of::mark_b());
    let space = cases.process();
    let ran = run(&space, target, 0, 0);
    report(
        "a second virtual CPU on another host thread runs the guest's level",
        called(&ran, MARK_B),
        format_args!("cpu {} x0 {:#x}", floor::cpu(), ran.registers.x[0]),
    );
}

/// Decision 10's option C: does the counter the framework emulates count EL0 instructions and raise its
/// overflow interrupt in the inside, and how late. Recorded, not a pass condition.
fn counter_trial(mut floor: Floor) {
    floor.interrupts(0);
    let mut cases = Cases {
        floor,
        next_code: Region::Code.base(),
        next_guest: GUEST,
        next_asid: 0,
    };
    let spin = cases.code_page(code_of::spin());
    let space = cases.process();
    let start = (1u64 << u32::BITS) - TRIAL_INSTRUCTIONS;
    // SAFETY: the performance counters only.
    unsafe {
        asm!(
            "msr pmcr_el0, {pmcr}",
            "msr pmevtyper0_el0, {kind}",
            "msr pmevcntr0_el0, {start}",
            "msr pmintenset_el1, {counter}",
            "msr pmcntenset_el0, {counter}",
            "isb",
            pmcr = in(reg) (PmcrEl0::E | PmcrEl0::P).bits(),
            kind = in(reg) EXCLUDE_EL1 | INSTRUCTIONS_RETIRED,
            start = in(reg) start,
            counter = in(reg) COUNTER_0,
            options(nostack),
        );
    }
    let (written, control): (u64, u64);
    // SAFETY: reads the counters' registers only.
    unsafe {
        asm!("mrs {0}, pmevcntr0_el0", "mrs {1}, pmcr_el0", out(reg) written, out(reg) control, options(nostack))
    };
    floor::arm_timer(counter() + TRIAL_TIMEOUT_TICKS);
    let ran = run(&space, spin, 0, 0);
    let id = cases.floor.take_interrupt();
    floor::disarm_timer();
    let count: u64;
    // SAFETY: reads the counter only.
    unsafe { asm!("mrs {0}, pmevcntr0_el0", out(reg) count, options(nostack)) };
    let retired = ran.registers.x[SPIN_COUNTER] * SPIN_INSTRUCTIONS;
    let by_counter = id == Some(cases.floor.start.counter_interrupt as u32);
    report(
        "counter trial, instructions (option C, recorded only)",
        true,
        format_args!(
            "event counter 0 written {start:#x} reads back {written:#x}, PMCR_EL0 {control:#x} (N, the event counters, {}); interrupt {id:?} ({}), counter after {count:#x}, EL0 instructions retired {retired}",
            PmcrEl0::from_bits_retain(control).n(),
            if by_counter {
                "the counter's overflow"
            } else {
                "not the counter's overflow"
            },
        ),
    );
    cycle_trial(&mut cases, spin, &space);
}

/// The same with the cycle counter, which bounds instructions from above by the core's widest retire.
fn cycle_trial(cases: &mut Cases, spin: u64, space: &AddressSpace) {
    let start = 0u64.wrapping_sub(TRIAL_CYCLES);
    let written: u64;
    // SAFETY: the performance counters only.
    unsafe {
        asm!(
            "msr pmccfiltr_el0, {filter}",
            "msr pmccntr_el0, {start}",
            "msr pmintenset_el1, {counter}",
            "msr pmcntenset_el0, {counter}",
            "isb",
            "mrs {written}, pmccntr_el0",
            filter = in(reg) EXCLUDE_EL1,
            start = in(reg) start,
            counter = in(reg) CYCLE_COUNTER,
            written = out(reg) written,
            options(nostack),
        );
    }
    floor::arm_timer(counter() + TRIAL_TIMEOUT_TICKS);
    let ran = run(space, spin, 0, 0);
    let id = cases.floor.take_interrupt();
    floor::disarm_timer();
    let (after, overflowed, enabled): (u64, u64, u64);
    // SAFETY: reads the counters' registers only.
    unsafe {
        asm!(
            "mrs {0}, pmccntr_el0", "mrs {1}, pmovsclr_el0", "mrs {2}, pmintenset_el1",
            out(reg) after, out(reg) overflowed, out(reg) enabled, options(nostack),
        )
    };
    let retired = ran.registers.x[SPIN_COUNTER] * SPIN_INSTRUCTIONS;
    let by_counter = id == Some(cases.floor.start.counter_interrupt as u32);
    report(
        "counter trial, cycles (option C, recorded only)",
        true,
        format_args!(
            "cycle counter written {start:#x} reads back {written:#x}; interrupt {id:?} ({}), counter after {after:#x}, overflow flags {overflowed:#x}, overflow interrupt enables {enabled:#x}, EL0 instructions retired {retired} in {TRIAL_CYCLES} armed cycles",
            if by_counter {
                "the counter's overflow"
            } else {
                "not the counter's overflow"
            },
        ),
    );
}

#[unsafe(no_mangle)]
extern "C" fn floor_main(start: &Start) -> ! {
    let mut floor = Floor::boot(start);
    if start.cpu != 0 {
        second(floor);
    } else if start.el2 != 0 {
        counter_trial(floor);
    } else {
        floor.interrupts(RAISED);
        let mut cases = Cases {
            floor,
            next_code: Region::Code.base(),
            next_guest: GUEST,
            next_asid: 0,
        };
        cases.execute_only();
        cases.guest_no_execute();
        cases.misaligned_branch();
        cases.tagged();
        cases.stop_word();
        cases.register_page();
        cases.two_containers();
        cases.thread_views();
        cases.adjacent_pages();
        cases.block();
        cases.late_code();
        cases.step();
        cases.timer();
        cases.raised();
        cases.doorbell();
        cases.give_back();
        cases.second_cpu();
        cases.stops();
    }
    loop {
        floor::ask(Request::Done, &[REPORTS.load(Ordering::Relaxed)], &[]);
    }
}
