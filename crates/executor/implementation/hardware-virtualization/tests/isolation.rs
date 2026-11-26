//! The machine never allows more than the record: sequences of the kernel's calls on the inside's `NativeCpu`
//! (through the test program `native-exits`) after which a guest touches a page, and the boot registers the
//! inside's tables rely on. Each case is one finding of the enforcement hand-off (2026-11-24) that the
//! machine could allow more than the record.

use aarch64_cpu::registers::{ID_AA64MMFR0_EL1, TCR_EL1};
use wcontainers_executor_hardware_virtualization_inside::boot::boot_registers;

/// ID_AA64MMFR0_EL1 of a chip with 48 physical address bits and the given ASID width.
fn features(asid_bits: u64) -> u64 {
    ID_AA64MMFR0_EL1::PARange::Bits_48.value | asid_bits
}

/// TCR_EL1.AS is 1, sixteen-bit address-space identifiers, only where the chip has them: on a chip with eight,
/// the bit is RES0 and identifier 256 would tag the same entries as 0.
#[test]
fn identifiers_are_sixteen_bits_only_where_the_chip_has_sixteen() {
    let as_bit = TCR_EL1::AS::ASID16Bits.value;
    let eight = boot_registers(0, features(ID_AA64MMFR0_EL1::ASIDBits::Bits_8.value), false);
    let sixteen = boot_registers(0, features(ID_AA64MMFR0_EL1::ASIDBits::Bits_16.value), false);
    assert_eq!(eight.tcr & as_bit, 0, "eight-bit identifiers on an eight-bit chip");
    assert_eq!(sixteen.tcr & as_bit, as_bit, "sixteen-bit identifiers on a sixteen-bit chip");
}

#[cfg(host_hypervisor)]
#[path = "../../hardware-virtualization-inside/bin/native-exits/protocol.rs"]
mod protocol;

#[cfg(host_hypervisor)]
mod common;

#[cfg(host_hypervisor)]
mod inside {
    use wcontainers_abi::{Access, Exit, PAGE_SIZE, Permissions, RegisterBlock, Resume};
    use wcontainers_executor_hardware_virtualization::{
        Exit as VcpuExit, Host, Hypervisor, Loaded, Memory, Register, VirtualCpu, VirtualMachine,
        boot, link as link_page, routines, translate::Translator,
    };
    use wcontainers_executor_hardware_virtualization_inside::layout::BLOCK_SIZE;
    use wcontainers_executor_hardware_virtualization_inside::link::{self, Fatal, Link, Start};
    use wcontainers_executor_hardware_virtualization_inside::{
        routines as inside_routines, translation,
    };
    use zerocopy::{FromBytes, IntoBytes};

    use crate::common::{MEMORY, ONE_CPU, be_signed, build, look_ahead, machine};
    use crate::protocol::{self, ExitWords, Order, OrderKind, ResumeWords};

    /// The guest's code page.
    const CODE: u64 = 0x1000_0000;
    /// A block-sized range of guest memory, aligned to a block.
    const BLOCK: u64 = 0x4000_0000;
    /// A page that shows a shared block.
    const SHOWN: u64 = 0x2000_0000;
    /// `str x1, [x0]`.
    const STORE: u32 = 0xf900_0001;
    /// `ldr x1, [x0]`.
    const LOAD: u32 = 0xf940_0001;
    /// `brk #0`.
    const BREAKPOINT: u32 = 0xd420_0000;
    /// What the guest stores.
    const STORED: u64 = 0x1122_3344_5566_7788;
    /// What the first shared block holds, and what a second one dropped after it holds.
    const FIRST: u64 = 0x0f1f_2f3f_4f5f_6f7f;
    const SECOND: u64 = 0x5ec0_5ec0_5ec0_5ec0;
    const READ_WRITE: Permissions = Permissions::READ.union(Permissions::WRITE);
    const READ_EXECUTE: Permissions = Permissions::READ.union(Permissions::EXECUTE);

    /// The inside, with one process and thread on its `NativeCpu`.
    struct Inside {
        cpu: <<Host as Hypervisor>::VirtualMachine as VirtualMachine>::Cpu,
        _machine: <Host as Hypervisor>::VirtualMachine,
        memory: Memory,
        loaded: Loaded,
        translator: Translator,
        warnings: Vec<String>,
    }

    impl Inside {
        fn start() -> Inside {
            be_signed();
            let program = build("native-exits");
            let (machine, memory, loaded, start) = machine(&program, MEMORY, ONE_CPU);
            let mut cpu = machine.cpu().expect("a virtual CPU");
            boot(&mut cpu, &memory, &loaded, 0, Start { ..start }).expect("booting");
            let mut inside = Inside {
                cpu,
                _machine: machine,
                memory,
                loaded,
                translator: Translator::new(look_ahead(), None),
                warnings: Vec::new(),
            };
            inside.serve();
            inside.order(OrderKind::Begin, 0, 0, 0, &[]);
            inside
        }

        fn link(&self) -> &'static mut Link {
            link_page(&self.memory, &self.loaded, 0)
        }

        fn order(&mut self, kind: OrderKind, address: u64, length: u64, value: u64, bytes: &[u8]) -> Vec<u8> {
            let link = self.link();
            link.put(&Order {
                kind: kind as u64,
                address,
                length,
                value,
            });
            link.head.length = bytes.len() as u64;
            link.bytes[..bytes.len()].copy_from_slice(bytes);
            self.serve()
        }

        fn word(&mut self, kind: OrderKind, address: u64, length: u64, value: u64) -> u64 {
            let given = self.order(kind, address, length, value, &[]);
            u64::read_from_bytes(&given).expect("one word")
        }

        fn serve(&mut self) -> Vec<u8> {
            loop {
                match self.cpu.run().expect("running") {
                    VcpuExit::Doorbell => {}
                    other => panic!(
                        "the inside stopped: {other:?} at {:#x}",
                        self.cpu.get(Register::Pc).unwrap_or_default()
                    ),
                }
                let link = self.link();
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
                        link::Request::GiveBack | link::Request::Carry | link::Request::Fetch => {
                            panic!("not asked here")
                        }
                    }
                } else if translation::Request::from_word(word).is_some() {
                    self.translator.answer(link);
                } else if inside_routines::Request::from_word(word).is_some() {
                    routines::answer(link, &self.memory);
                } else {
                    match protocol::Request::from_word(word).expect("a request of the set") {
                        protocol::Request::Next => return link.data().to_vec(),
                        protocol::Request::Warned => self
                            .warnings
                            .push(String::from_utf8_lossy(link.data()).into_owned()),
                    }
                }
            }
        }

        fn protect(&mut self, at: u64, length: u64, permissions: Permissions) {
            self.order(OrderKind::Protect, at, length, permissions.bits().into(), &[]);
        }

        /// Whether a narrowing or widening was refused.
        fn try_protect(&mut self, at: u64, length: u64, permissions: Permissions) -> bool {
            self.word(OrderKind::TryProtect, at, length, permissions.bits().into()) != 0
        }

        fn write(&mut self, at: u64, bytes: &[u8]) {
            self.order(OrderKind::Write, at, 0, 0, bytes);
        }

        fn code(&mut self, words: &[u32]) {
            self.protect(CODE, PAGE_SIZE, READ_EXECUTE);
            self.write(CODE, words.as_bytes());
        }

        /// Runs the code from its start with x0 and x1 set; gives the exit and x1 after.
        fn run(&mut self, x0: u64, x1: u64) -> (Exit, u64) {
            let mut block = RegisterBlock {
                pc: CODE,
                ..RegisterBlock::ZERO
            };
            block.x[0] = x0;
            block.x[1] = x1;
            self.order(OrderKind::SetRegisters, 0, 0, 0, block.as_bytes());
            let (kind, value) = ResumeWords::words(Resume::FromRegisters);
            let given = self.order(OrderKind::Run, kind, 0, value, &[]);
            let (exit, rest) = ExitWords::read_from_prefix(&given).expect("an exit");
            let after = RegisterBlock::read_from_bytes(rest).expect("a register block");
            (exit.exit(), after.x[1])
        }
    }

    /// Finding 1: a narrowing that needs a frame to split a block and finds none, not even by breaking a
    /// reservation. The refusal must leave the record as the machine has it, so that the same narrowing, once
    /// frames are back, makes the page fault.
    #[test]
    fn a_narrowing_refused_for_want_of_frames_never_leaves_the_page_wider() {
        let mut inside = Inside::start();
        inside.code(&[STORE, BREAKPOINT]);
        let second = BLOCK + BLOCK_SIZE;
        inside.protect(BLOCK, 2 * BLOCK_SIZE, READ_WRITE);
        // Every page of each block takes its frame of one run, so one block entry maps each.
        for page in (BLOCK..BLOCK + 2 * BLOCK_SIZE).step_by(PAGE_SIZE as usize) {
            inside.write(page, &[1]);
        }
        // The first narrowing may find a frame by breaking the code page's reservation; the second finds none.
        let taken = inside.word(OrderKind::TakeEveryFrame, 0, 0, 0);
        let first_refused = inside.try_protect(BLOCK, PAGE_SIZE, Permissions::READ);
        let more = inside.word(OrderKind::TakeEveryFrame, 0, 0, 0);
        let refused = inside.try_protect(second, PAGE_SIZE, Permissions::READ);
        inside.order(OrderKind::GiveFramesBack, 0, 0, 0, &[]);
        println!(
            "took {taken} frames, then {more}; the narrowings with none left were refused: {first_refused}, {refused}"
        );
        assert!(refused, "a narrowing with no frame left anywhere is refused");
        assert!(
            !inside.try_protect(second, PAGE_SIZE, Permissions::READ),
            "the same narrowing with frames left"
        );
        let (exit, _) = inside.run(second, STORED);
        assert_eq!(
            exit,
            Exit::MemoryFault {
                address: second,
                access: Access::Write
            },
            "the guest wrote a page the record says is read-only"
        );
    }

    /// Finding 2: a shared block dropped while a memory still shows it. Its frame must not go to another
    /// block while the memory's page still maps it.
    #[test]
    fn a_block_dropped_while_shown_never_gives_its_frame_to_another() {
        let mut inside = Inside::start();
        inside.code(&[LOAD, BREAKPOINT]);
        inside.protect(SHOWN, PAGE_SIZE, READ_WRITE);
        let first = inside.word(OrderKind::SharedNew, 0, PAGE_SIZE, 0);
        inside.order(OrderKind::SharedMap, SHOWN, PAGE_SIZE, first, &[]);
        inside.order(OrderKind::SharedWrite, 0, 0, first, &FIRST.to_le_bytes());
        // The guest's load maps the block's frame at the page.
        let (exit, loaded) = inside.run(SHOWN, 0);
        assert_eq!(exit, Exit::Breakpoint { comment: 0 });
        assert_eq!(loaded, FIRST, "the guest reads the block it shows");
        inside.order(OrderKind::SharedDrop, 0, 0, first, &[]);
        let second = inside.word(OrderKind::SharedNew, 0, PAGE_SIZE, 0);
        inside.order(OrderKind::SharedWrite, 0, 0, second, &SECOND.to_le_bytes());
        let (exit, loaded) = inside.run(SHOWN, 0);
        assert_eq!(exit, Exit::Breakpoint { comment: 0 });
        assert_ne!(loaded, SECOND, "the guest read a block its memory never showed");
        assert_eq!(loaded, FIRST, "the page still shows what the dropped block held");
    }
}
