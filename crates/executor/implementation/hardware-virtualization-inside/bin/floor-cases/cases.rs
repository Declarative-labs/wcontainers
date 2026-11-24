//! The cases. Each prints one report: its name, whether it passed, and what it saw.

use core::arch::global_asm;
use core::fmt::Write as _;
use core::time::Duration;

use aarch64_cpu::registers::{ESR_EL1, FAR_EL1, MDSCR_EL1, Read, SPSR_EL1, TPIDRRO_EL0, Write};
use wcontainers_abi::PAGE_SIZE;
use wcontainers_executor_hardware_virtualization_inside::floor::tables::large_entry_splits;
use wcontainers_executor_hardware_virtualization_inside::floor::{
    self, AddressSpace, Floor, Frames, Full, Taken, UserRegisters, code, counter,
    counter_frequency, enter_user, guest_read_only, guest_read_write, heap, make_code_visible,
    process_code, register_page, set_stop_word, with_frames, write_frame,
};
use wcontainers_executor_hardware_virtualization_inside::layout::{
    BLOCK_LEVEL, BLOCK_SIZE, LEAF_LEVEL, MIRROR, Region, TOP_LEVEL, level_span, linear,
};
use wcontainers_executor_hardware_virtualization_inside::link::{GiveBack, Start};
use wcontainers_executor_hardware_virtualization_inside::syndrome::FAULT_STATUS;
use wcontainers_processor_architecture_arm64::syndrome::WRITE_NOT_READ;

use crate::protocol::{
    Case, Done, Echo, Footprint, Lower, Measurement, Nothing, RaiseLater, Report, StartCpu,
    StopTest, Write as WriteCode,
};

/// Marks the cases put in registers and memory to tell their pages apart.
const MARK_A: u64 = 0xa1;
const MARK_B: u64 = 0xb2;

/// A tag in a pointer's top byte.
const TAG: u64 = 0x5a;

/// Where a pointer's tag sits: its top byte.
const TAG_SHIFT: u32 = u64::BITS - u8::BITS;

/// Guest addresses the cases map their data at.
const GUEST: u64 = 0x10_0000;

/// Times the cases that switch table roots go back and forth.
const ROUNDS: u64 = 24;

/// Doorbell round trips timed.
const DOORBELLS: u64 = 1000;

/// Interrupt numbers the host raises, from the first one it may.
const RAISED: u32 = 1;

/// Microseconds the host waits before raising an interrupt.
const RAISE_DELAY_US: u64 = 200;

/// Counter ticks before a timer case's interrupt.
const TIMER_TICKS: u64 = 2400;

/// Blocks of memory the give-back case touches and gives back.
const GIVE_BACK_BLOCKS: u64 = 32;

/// Instructions the software step case steps through before its system call.
const STEPS: u64 = 8;

/// The permission fault status code, plus the level the fault happened at.
const PERMISSION_FAULT: u64 = 0b00_1100;

/// The second virtual CPU's address-space identifiers start here, apart from the first's.
const SECOND_CPU_ASIDS: u16 = 0x100;

/// The register the spin snippet counts its rounds in.
const SPIN_COUNTER: usize = 19;

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

/// Sends `case`'s report: what it saw.
fn report(case: Case, passed: bool, text: core::fmt::Arguments) {
    floor::send(
        &Report {
            case: case as u64,
            passed: u64::from(passed),
        },
        text,
    );
}

/// Sends `case`'s measurement: what was measured, and the figures.
fn measured(case: Case, text: core::fmt::Arguments) {
    floor::send(&Measurement { case: case as u64 }, text);
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

/// Runs `f` over the frames; the cases are sized to fit guest-physical memory.
fn frames<T>(f: impl FnOnce(&mut Frames) -> Result<T, Full>) -> T {
    with_frames(f).expect("the cases fit guest-physical memory")
}

impl Cases {
    /// A page of the code area holding `bytes`, made visible.
    fn code_page(&mut self, bytes: &[u8]) -> u64 {
        let va = self.next_code;
        self.next_code += PAGE_SIZE;
        let global = self.floor.global;
        frames(|frames| {
            let frame = frames.take()?;
            write_frame(frame, 0, bytes);
            global.map(frames, va, frame, code())
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
        frames(|frames| AddressSpace::new(frames, &global, asid))
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
            Case::ExecuteOnly,
            passed,
            format_args!(
                "execute-only code runs and refuses loads and stores: run x0 {:#x}; load EC {:#x} FSC {:#x} FAR {:#x}; store EC {:#x} FSC {:#x} write {} FAR {:#x}; own page FAR {:#x}",
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

    fn system_call_out(&mut self) {
        let target = self.code_page(code_of::mark_a());
        let space = self.process();
        let ran = run(&space, target, 0, 0);
        let mut passed = called(&ran, MARK_A);
        if passed {
            let echoed = floor::ask(
                &Echo {
                    value: ran.registers.x[0],
                },
                &[],
            )
            .value;
            passed = echoed == MARK_A;
        }
        report(
            Case::SystemCallOut,
            passed,
            format_args!(
                "a guest-level system call reaches the host through the floor's doorbell: system call x0 {:#x}",
                ran.registers.x[0]
            ),
        );
    }

    fn guest_no_execute(&mut self) {
        let space = self.process();
        let page = self.guest_page();
        frames(|frames| {
            let frame = frames.take()?;
            write_frame(frame, 0, code_of::mark_a());
            space.user.map(frames, page, frame, guest_read_write())
        });
        let ran = run(&space, page, 0, 0);
        let passed = ran.class == class(ESR_EL1::EC::Value::InstrAbortLowerEL)
            && ran.status == PERMISSION_FAULT + u64::from(LEAF_LEVEL)
            && ran.far == page
            && ran.registers.pc == page;
        report(
            Case::GuestNoExecute,
            passed,
            format_args!(
                "guest pages refuse execution: EC {:#x} FSC {:#x} FAR {:#x} ELR {:#x}",
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
        for offset in 1..size_of::<u32>() as u64 {
            let ran = run(&space, branch, target + offset, 0);
            passed &= ran.class == class(ESR_EL1::EC::Value::PCAlignmentFault)
                && ran.far == target + offset
                && ran.registers.pc == target + offset;
            let _ = out.write_fmt(format_args!(
                "offset {offset}: EC {:#x} FAR {:#x} ELR {:#x}; ",
                ran.class, ran.far, ran.registers.pc
            ));
        }
        report(
            Case::MisalignedBranch,
            passed,
            format_args!("a misaligned br faults at its target: {}", out.as_str()),
        );
    }

    fn tagged(&mut self) {
        let load = self.code_page(code_of::load());
        let branch = self.code_page(code_of::branch());
        let space = self.process();
        let page = self.guest_page();
        frames(|frames| {
            let frame = frames.take()?;
            write_frame(frame, 0, &MARK_A.to_le_bytes());
            space.user.map(frames, page, frame, guest_read_write())
        });
        let tagged = page | (TAG << TAG_SHIFT);
        let loaded = run(&space, load, tagged, 0);
        let branched = run(&space, branch, tagged, 0);
        let passed = called(&loaded, MARK_A)
            && branched.class == class(ESR_EL1::EC::Value::InstrAbortLowerEL)
            && branched.status == PERMISSION_FAULT + u64::from(LEAF_LEVEL)
            && branched.far == page;
        report(
            Case::Tagged,
            passed,
            format_args!(
                "tagged loads and branches ignore the top byte (TBI0): load x0 {:#x}; branch to {tagged:#x}: EC {:#x} FSC {:#x} FAR {:#x}",
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
            Case::StopWord,
            passed,
            format_args!(
                "TPIDRRO_EL0 is written at EL1, read at EL0, and an EL0 write is undefined: read {:#x}; write EC {:#x} at {:#x}; after {after:#x}",
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
        frames(|frames| {
            let frame = frames.take()?;
            write_frame(frame, 0, &mark.to_le_bytes());
            space.inside.map(frames, va, frame, attributes)?;
            Ok(frame)
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
            Case::RegisterPage,
            passed,
            format_args!(
                "the register page is readable and writable at EL0: load {:#x}; store EC {:#x}; after {after:#x}",
                loaded.registers.x[0], stored.class
            ),
        );
    }

    fn protect_inside_page(&mut self) {
        let store = self.code_page(code_of::store());
        let space = self.process();
        let va = Region::Threads.base();
        self.inside_page(&space, va, MARK_A, register_page());
        let before = run(&space, store, va, MARK_B);
        frames(|frames| space.inside.protect(frames, va, guest_read_only()));
        let after = run(&space, store, va, MARK_A);
        let passed = called(&before, 0) && refused(&after, va, true);
        report(
            Case::ProtectInsidePage,
            passed,
            format_args!(
                "a page of the inside's half made read-only after use refuses at once: store before EC {:#x}; after EC {:#x} FSC {:#x} FAR {:#x}",
                before.class, after.class, after.status, after.far
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
            frames(|frames| {
                let frame = frames.take()?;
                write_frame(frame, 0, mirror_code);
                space.inside.map(frames, MIRROR, frame, process_code())
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
                    let _ = out.write_fmt(format_args!(
                        "round {round} asid {}: mirror {:#x} registers {:#x} heap {heap:#x}",
                        space.asid, mirror.registers.x[0], registers.registers.x[0]
                    ));
                }
                passed &= ok;
            }
        }
        report(
            Case::TwoContainers,
            passed,
            format_args!(
                "two containers' processes see only their own mirror, register page and heap: {ROUNDS} rounds of switching with no invalidation {}",
                out.as_str()
            ),
        );
    }

    fn thread_table_roots(&mut self) {
        let store = self.code_page(code_of::store());
        let load = self.code_page(code_of::load());
        let process = self.process();
        let page = self.guest_page();
        self.next_asid += 1;
        let writer_asid = self.next_asid;
        self.next_asid += 1;
        let reader_asid = self.next_asid;
        let (writer, reader) = frames(|frames| {
            let frame = frames.take()?;
            let writer = process.thread_address_space(frames, writer_asid)?;
            let reader = process.thread_address_space(frames, reader_asid)?;
            writer.user.map(frames, page, frame, guest_read_write())?;
            reader.user.map(frames, page, frame, guest_read_only())?;
            Ok((writer, reader))
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
            Case::ThreadTableRoots,
            passed,
            format_args!(
                "two threads of one process with their own table roots see one page writable and read-only: {ROUNDS} rounds, identifiers {writer_asid} and {reader_asid}, no invalidation"
            ),
        );
    }

    fn shared_inside_half(&mut self) {
        let store = self.code_page(code_of::store());
        let process = self.process();
        self.next_asid += 1;
        let first_asid = self.next_asid;
        self.next_asid += 1;
        let second_asid = self.next_asid;
        let (first, second) = frames(|frames| {
            Ok((
                process.thread_address_space(frames, first_asid)?,
                process.thread_address_space(frames, second_asid)?,
            ))
        });
        let va = Region::Threads.base();
        self.inside_page(&process, va, MARK_A, register_page());
        let mut passed = true;
        for round in 0..ROUNDS {
            let stored_first = run(&first, store, va, round);
            let stored_second = run(&second, store, va, round);
            frames(|frames| process.inside.protect(frames, va, guest_read_only()));
            let refused_second = run(&second, store, va, MARK_B);
            frames(|frames| process.inside.protect(frames, va, register_page()));
            passed &= called(&stored_first, 0)
                && called(&stored_second, 0)
                && refused(&refused_second, va, true);
        }
        report(
            Case::SharedInsideHalf,
            passed,
            format_args!(
                "two table roots of one process share the inside's half: a page made read-only after both stored refuses the other root's store at once, {ROUNDS} rounds, identifiers {first_asid} and {second_asid}"
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
        let frame = frames(|frames| {
            let frame = frames.take_block()?;
            space.user.map(frames, first, frame, guest_read_write())?;
            space
                .user
                .map(frames, second, frame + PAGE_SIZE, guest_read_only())?;
            Ok(frame)
        });
        let same_host_page = frame / host_page == (frame + PAGE_SIZE) / host_page;
        // A host page of one guest page holds no two of them; a larger one must hold both frames of the block.
        let shared_where_possible = same_host_page == (host_page > PAGE_SIZE);
        let wrote_first = run(&space, store, first, MARK_A);
        let wrote_second = run(&space, store, second, MARK_B);
        let read_second = run(&space, load, second, 0);
        let passed = shared_where_possible
            && called(&wrote_first, 0)
            && refused(&wrote_second, second, true)
            && called(&read_second, 0);
        report(
            Case::AdjacentPages,
            passed,
            format_args!(
                "two adjacent guest pages in one host page keep their own permissions: frames {frame:#x} and {:#x}, host page {host_page}, one host page {same_host_page}; second store EC {:#x} FSC {:#x} FAR {:#x}",
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
        frames(|frames| {
            let block = frames.take_block()?;
            space
                .user
                .map_block(frames, base, block, guest_read_write())
        });
        let pages = BLOCK_SIZE / PAGE_SIZE;
        let level_before = space.user.find(base).map(|(_, level)| level);
        let mut passed = level_before == Some(BLOCK_LEVEL);
        for page in 0..pages {
            passed &= called(&run(&space, store, base + page * PAGE_SIZE, page), 0);
        }
        let changed = base + (pages / 2) * PAGE_SIZE;
        let splits = large_entry_splits();
        frames(|frames| space.user.protect(frames, changed, guest_read_only()));
        let level_after = space.user.find(base).map(|(_, level)| level);
        let counted = large_entry_splits() - splits;
        passed &= level_after == Some(LEAF_LEVEL) && counted == 1;
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
            Case::Block,
            passed,
            format_args!(
                "a block entry reads and writes at EL0, and after one page's change only that page refuses: level before {level_before:?} after {level_after:?}, splits counted {counted}; changed page {changed:#x}: EC {:#x} FAR {:#x}",
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
            floor::ask(&WriteCode { at: frame + half }, bytes);
            make_code_visible(page + half, bytes.len() as u64);
            let late = run(&space, page + half, 0, 0);
            if !called(&late, mark) {
                wrong += 1;
            }
        }
        passed &= wrong == 0;
        report(
            Case::LateCode,
            passed,
            format_args!(
                "code the monitor writes next to code that ran runs as written after maintenance: {ROUNDS} rounds, {wrong} ran stale"
            ),
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
                let _ = out.write_fmt(format_args!(
                    "step {step}: EC {:#x} ELR {:#x}",
                    ran.class, ran.registers.pc
                ));
            }
            passed &= ok;
            registers = ran.registers;
        }
        registers.pstate |= SPSR_EL1::SS::SET.value;
        let last = resume(&space, registers);
        MDSCR_EL1.modify(MDSCR_EL1::SS::SoftwareStepDisabled);
        passed &= called(&last, STEPS);
        report(
            Case::Step,
            passed,
            format_args!(
                "EL1 software step of EL0 stops after each instruction: {STEPS} steps, then the system call gives EC {:#x}; {}",
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
            Case::Timer,
            passed,
            format_args!(
                "the virtual timer's interrupt lands in the inside: {ROUNDS} interrupts, nanoseconds after the deadline: median {} max {}",
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
            floor::ask(
                &RaiseLater {
                    interrupt,
                    delay_us: RAISE_DELAY_US,
                },
                &[],
            );
            let ran = run(&space, spin, 0, 0);
            let now = counter();
            let id = self.floor.take_interrupt();
            let raised_at = floor::ask(&Lower { interrupt }, &[]).counter;
            passed &=
                ran.taken == Taken::Interrupt && id == Some(interrupt as u32) && now >= raised_at;
            *slot = ticks_to_nanos(now.saturating_sub(raised_at));
        }
        delay.sort_unstable();
        report(
            Case::Raised,
            passed,
            format_args!(
                "an interrupt the host raises from another thread lands in the inside: {ROUNDS} interrupts, nanoseconds from raise to the inside: median {} max {}",
                delay[delay.len() / 2],
                delay[delay.len() - 1]
            ),
        );
    }

    fn doorbell(&mut self) {
        let start = counter();
        for _ in 0..DOORBELLS {
            floor::ask(&Nothing {}, &[]);
        }
        let each = ticks_to_nanos(counter() - start) / DOORBELLS;
        measured(
            Case::Doorbell,
            format_args!("the doorbell's round trip: {DOORBELLS} round trips, {each} ns each"),
        );
    }

    fn give_back(&mut self) {
        let length = GIVE_BACK_BLOCKS * BLOCK_SIZE;
        let first = frames(|frames| {
            let first = frames.take_block()?;
            for _ in 1..GIVE_BACK_BLOCKS {
                frames.take_block()?;
            }
            Ok(first)
        });
        let mut at = first;
        while at < first + length {
            write_frame(at, 0, &MARK_A.to_le_bytes());
            at += PAGE_SIZE;
        }
        let before = floor::ask(&Footprint {}, &[]).bytes;
        floor::ask(&GiveBack { at: first, length }, &[]);
        let after = floor::ask(&Footprint {}, &[]).bytes;
        // SAFETY: the given-back memory is still mapped, and reads as zeros.
        let reads = unsafe { core::ptr::read_volatile(linear(first + length / 2) as *const u64) };
        let fell = before.saturating_sub(after);
        let passed = fell >= length / 2 && reads == 0;
        report(
            Case::GiveBack,
            passed,
            format_args!(
                "memory given back through the link lowers the process's footprint: gave back {length} bytes; footprint {before} to {after}, fell {fell}; reads {reads:#x} after"
            ),
        );
    }

    fn second_cpu(&mut self) {
        floor::ask(&StartCpu { cpu: 1 }, &[]);
    }

    fn stops(&mut self) {
        let spin = self.code_page(code_of::spin());
        let space = self.process();
        let interrupt = self.floor.start.first_raised_interrupt;
        let end = spin + code_of::spin().len() as u64;
        floor::ask(
            &StopTest {
                interrupt,
                loop_start: spin,
                loop_end: end,
                counter: SPIN_COUNTER as u64,
            },
            &[],
        );
        let ran = run(&space, spin, 0, 0);
        let id = self.floor.take_interrupt();
        floor::ask(&Lower { interrupt }, &[]);
        let passed = ran.taken == Taken::Interrupt && id == Some(interrupt as u32);
        report(
            Case::Stops,
            passed,
            format_args!(
                "the spin stopped from another host thread ends by a raised interrupt: {} rounds of the spin",
                ran.registers.x[SPIN_COUNTER]
            ),
        );
    }
}

/// Code written into `space`'s own pages made visible, with `space` switched to.
fn make_code_visible_in(space: &AddressSpace, va: u64, length: u64) {
    space.switch();
    make_code_visible(va, length);
}

fn ticks_to_nanos(ticks: u64) -> u64 {
    let frequency = u32::try_from(counter_frequency()).expect("the counter frequency as a divisor");
    (Duration::from_secs(ticks) / frequency).as_nanos() as u64
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
        next_code: Region::Code.base() + level_span(TOP_LEVEL + 1),
        next_guest: GUEST,
        next_asid: SECOND_CPU_ASIDS,
    };
    let target = cases.code_page(code_of::mark_b());
    let space = cases.process();
    let ran = run(&space, target, 0, 0);
    report(
        Case::SecondCpu,
        called(&ran, MARK_B),
        format_args!(
            "a second virtual CPU on another host thread runs the guest's level: cpu {} x0 {:#x}",
            floor::cpu(),
            ran.registers.x[0]
        ),
    );
}

#[unsafe(no_mangle)]
extern "C" fn floor_main(start: &Start) -> ! {
    let mut floor = Floor::boot(start);
    if start.cpu != 0 {
        second(floor);
    } else {
        floor.interrupts(RAISED);
        let mut cases = Cases {
            floor,
            next_code: Region::Code.base(),
            next_guest: GUEST,
            next_asid: 0,
        };
        cases.execute_only();
        cases.system_call_out();
        cases.guest_no_execute();
        cases.misaligned_branch();
        cases.tagged();
        cases.stop_word();
        cases.register_page();
        cases.protect_inside_page();
        cases.two_containers();
        cases.thread_table_roots();
        cases.shared_inside_half();
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
        floor::ask(&Done {}, &[]);
    }
}
