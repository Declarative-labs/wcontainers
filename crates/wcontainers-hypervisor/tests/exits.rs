//! N3's first programs on the native CPU side: static ARM64 programs run by the inside's `NativeCpu` (through
//! the test program `native-exits`) and by the WebAssembly output's CPU side, every exit and every register
//! compared, each system call answered alike on both sides. It runs twice, without and with the progress
//! count, each run a signed copy of itself.
//!
//! `WCONTAINERS_EXITS_EXACT` (0 or 1) runs one of the two only.

#[cfg(target_os = "macos")]
#[path = "../../wcontainers-inside/src/bin/native-exits/protocol.rs"]
mod protocol;

#[cfg(target_os = "macos")]
mod common;

#[cfg(target_os = "macos")]
fn main() {
    macos::main();
}

#[cfg(not(target_os = "macos"))]
fn main() {
    println!("the exits test needs Hypervisor.framework; nothing to run on this host");
}

#[cfg(target_os = "macos")]
mod macos {
    use std::collections::BTreeMap;
    use std::process::Command;
    use std::time::Instant;

    use linux_raw_sys::auxvec::{AT_NULL, AT_PAGESZ};
    use linux_raw_sys::errno::ENOSYS;
    use linux_raw_sys::general::{__NR_exit, __NR_exit_group, __NR_write, __NR_writev, iovec};
    use object::elf::{PF_R, PF_W, PF_X};
    use object::{Object, ObjectSegment, SegmentFlags};
    use wcontainers_abi::{
        Cpu, Exit, MemoryId, PAGE_SIZE, Permissions, RegisterBlock, Resume, ThreadId, Warning,
    };
    use wcontainers_arm64::Arm64;
    use wcontainers_arm64::thread_state::STACK_ALIGNMENT;
    use wcontainers_hypervisor::{
        Exit as VcpuExit, Host, Hypervisor, Loaded, Memory, Register, VirtualCpu, VirtualMachine,
        boot, link as link_page, routines, translate::Translator,
    };
    use wcontainers_inside::link::{self, Fatal, LINK_BYTES, Link, Start};
    use wcontainers_inside::reason::IDENTIFIERS_RAN_OUT;
    use wcontainers_inside::{routines as inside_routines, translation};
    use wcontainers_runtime::{Answer, Carry, Execution, Leaving, Turn};
    use wcontainers_wasmtime::{Compiler, CpuSide, LookAhead, Optimise};
    use zerocopy::{FromBytes, IntoBytes};

    use crate::common::{
        CLibrary, KERNEL_TEST_GUEST, MEMORY, ONE_CPU, OWN_GUEST, be_signed, build, compile,
        load_average, look_ahead, machine, setting,
    };
    use crate::protocol::{self, ExitWords, Order, OrderKind, ResumeWords, Warned};

    /// The test program inside.
    const PROGRAM: &str = "native-exits";

    /// Runs one of the two modes only, when set: 1 with the progress count, 0 without.
    const EXACT_VARIABLE: &str = "WCONTAINERS_EXITS_EXACT";

    /// The guest's stack's size; its top is the end of the smaller address space of the two sides.
    const STACK_BYTES: u64 = 64 * PAGE_SIZE;

    /// Each program: its directory under this crate and its name.
    const PROGRAMS: &[(&str, &str, CLibrary)] = &[
        (KERNEL_TEST_GUEST, "hello", CLibrary::Without),
        (OWN_GUEST, "exits", CLibrary::Without),
        (OWN_GUEST, "libc", CLibrary::Musl),
    ];

    /// Address-space identifiers: every one the hardware's 16 bits name.
    const IDENTIFIERS: u64 = 1 << u16::BITS;

    /// The standard output's and standard error's descriptors.
    const DESCRIPTORS: usize = 3;

    fn main_mode() -> Option<bool> {
        std::env::var_os(EXACT_VARIABLE)?;
        Some(setting(EXACT_VARIABLE, 0) != 0)
    }

    pub fn main() {
        let Some(exact) = main_mode() else {
            for exact in [false, true] {
                let status = Command::new(std::env::current_exe().expect("the test's own path"))
                    .args(std::env::args_os().skip(1))
                    .env(EXACT_VARIABLE, u8::from(exact).to_string())
                    .status()
                    .expect("running the test for one mode");
                assert!(status.success(), "the run with exact {exact} failed");
            }
            return;
        };
        be_signed();
        let started = Instant::now();
        let mut native = NativeSide::start(exact);
        let compiler = Compiler::<Arm64>::new(None);
        let mut failed = 0;
        let mut compare_one = |native: &mut NativeSide, name: &str, image: &[u8]| {
            let mut wasm = WasmSide::new(&compiler);
            native.begin(exact);
            match compare(image, name, native, &mut wasm) {
                Ok(outcome) => println!("{name} (exact {exact}): {outcome}"),
                Err(why) => {
                    failed += 1;
                    println!("{name} (exact {exact}) FAILED: {why}");
                }
            }
        };
        for &(directory, name, library) in PROGRAMS {
            compare_one(&mut native, name, &compile(directory, name, library));
        }
        // More memories one after another than there are address-space identifiers: a new generation of them,
        // and every memory's tables given back, or they would not fit the machine's memory.
        let (directory, name, library) = PROGRAMS[0];
        for _ in 0..IDENTIFIERS {
            native.begin(exact);
        }
        compare_one(&mut native, name, &compile(directory, name, library));
        assert!(
            native.warnings.contains_key(IDENTIFIERS_RAN_OUT),
            "{IDENTIFIERS_RAN_OUT} after {IDENTIFIERS} memories"
        );
        println!(
            "exits, exact {exact}, {:?}: {} programs, {failed} failed; {} native pieces; native slow paths {:?}; \
             in {:?}, load {}",
            native.translator.look_ahead(),
            PROGRAMS.len() + 1,
            native.translations,
            native.warnings,
            started.elapsed(),
            load_average()
        );
        assert_eq!(failed, 0);
    }

    /// A CPU side one process and thread are run on.
    trait Side {
        fn protect(&mut self, at: u64, length: u64, permissions: Permissions);
        fn write(&mut self, at: u64, bytes: &[u8]);
        fn read(&mut self, at: u64, length: u64) -> Vec<u8>;
        fn set_registers(&mut self, block: &RegisterBlock);
        fn run(&mut self, resume: Resume) -> (Exit, RegisterBlock);
    }

    /// The native CPU side, in the virtual machine.
    struct NativeSide {
        cpu: <<Host as Hypervisor>::VirtualMachine as VirtualMachine>::Cpu,
        _machine: <Host as Hypervisor>::VirtualMachine,
        memory: Memory,
        loaded: Loaded,
        /// Each slow path's count, as last warned.
        warnings: BTreeMap<String, u64>,
        /// Pieces translated.
        translations: u64,
        /// The monitor's translations.
        translator: Translator,
    }

    impl NativeSide {
        /// Boots the program and serves it until it asks for its first order.
        fn start(exact: bool) -> NativeSide {
            let program = build(PROGRAM);
            let (machine, memory, loaded, start) = machine(&program, MEMORY, ONE_CPU);
            let mut cpu = machine.cpu().expect("a virtual CPU");
            boot(&mut cpu, &memory, &loaded, 0, Start { ..start }).expect("booting");
            let mut side = NativeSide {
                cpu,
                _machine: machine,
                memory,
                loaded,
                warnings: BTreeMap::new(),
                translations: 0,
                translator: Translator::new(look_ahead()),
            };
            side.serve();
            side.begin(exact);
            side
        }

        fn begin(&mut self, exact: bool) {
            self.order(
                Order {
                    kind: OrderKind::Begin as u64,
                    value: exact.into(),
                    ..Order::default()
                },
                &[],
            );
        }

        fn link(&self) -> &'static mut Link {
            link_page(&self.memory, &self.loaded, 0)
        }

        /// Answers the inside's last [`protocol::Next`] with `order` and `bytes`, and gives back what the order
        /// gave.
        fn order(&mut self, order: Order, bytes: &[u8]) -> Vec<u8> {
            let link = self.link();
            link.put(&order);
            link.head.length = bytes.len() as u64;
            link.bytes[..bytes.len()].copy_from_slice(bytes);
            self.serve()
        }

        /// Runs the inside until it asks for its next order, answering everything else.
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
                        link::Request::GiveBack => panic!("no give-back here"),
                        link::Request::Carry | link::Request::Fetch => {
                            panic!("no bytes longer than a link here")
                        }
                    }
                } else if let Some(request) = translation::Request::from_word(word) {
                    self.translator.answer(link);
                    self.translations += u64::from(request == translation::Request::Translate);
                } else if inside_routines::Request::from_word(word).is_some() {
                    routines::answer(link, &self.memory);
                } else {
                    match protocol::Request::from_word(word).expect("a request of the set") {
                        protocol::Request::Next => return link.data().to_vec(),
                        protocol::Request::Warned => {
                            let Warned { count, .. } = link.get();
                            let reason = String::from_utf8_lossy(link.data()).into_owned();
                            self.warnings.insert(reason, count);
                        }
                    }
                }
            }
        }
    }

    impl Side for NativeSide {
        fn protect(&mut self, at: u64, length: u64, permissions: Permissions) {
            self.order(
                Order {
                    kind: OrderKind::Protect as u64,
                    address: at,
                    length,
                    value: permissions.bits().into(),
                },
                &[],
            );
        }

        fn write(&mut self, at: u64, bytes: &[u8]) {
            for (k, part) in bytes.chunks(LINK_BYTES).enumerate() {
                self.order(
                    Order {
                        kind: OrderKind::Write as u64,
                        address: at + (k * LINK_BYTES) as u64,
                        ..Order::default()
                    },
                    part,
                );
            }
        }

        fn read(&mut self, at: u64, length: u64) -> Vec<u8> {
            let mut out = Vec::new();
            while (out.len() as u64) < length {
                let take = (length - out.len() as u64).min(LINK_BYTES as u64);
                out.extend(self.order(
                    Order {
                        kind: OrderKind::Read as u64,
                        address: at + out.len() as u64,
                        length: take,
                        ..Order::default()
                    },
                    &[],
                ));
            }
            out
        }

        fn set_registers(&mut self, block: &RegisterBlock) {
            self.order(
                Order {
                    kind: OrderKind::SetRegisters as u64,
                    ..Order::default()
                },
                block.as_bytes(),
            );
        }

        fn run(&mut self, resume: Resume) -> (Exit, RegisterBlock) {
            let (kind, value) = ResumeWords::words(resume);
            let result = self.order(
                Order {
                    kind: OrderKind::Run as u64,
                    address: kind,
                    value,
                    ..Order::default()
                },
                &[],
            );
            let (exit, rest) = ExitWords::read_from_prefix(&result).expect("an exit");
            let block = RegisterBlock::read_from_bytes(rest).expect("a register block");
            (exit.exit(), block)
        }
    }

    /// The WebAssembly output's CPU side, as the runtime's loop runs it.
    struct WasmSide {
        cpu: CpuSide<Arm64>,
        memory: MemoryId,
        thread: ThreadId,
    }

    impl WasmSide {
        fn new(compiler: &std::sync::Arc<Compiler<Arm64>>) -> WasmSide {
            let mut cpu = CpuSide::new(compiler.clone(), LookAhead::None, Optimise::Speed, ());
            let memory = cpu.memory_new().expect("a memory");
            let thread = cpu.thread_new(memory).expect("a thread");
            WasmSide {
                cpu,
                memory,
                thread,
            }
        }
    }

    /// The loop's part: takes the one exit and leaves.
    #[derive(Default)]
    struct OneExit {
        exit: Option<Exit>,
    }

    impl Answer for OneExit {
        fn exit(&mut self, _cpu: &mut dyn Cpu, exit: Exit, _progress: u64) -> Carry {
            self.exit = Some(exit);
            Carry::Leave(Leaving::SystemAsked)
        }

        fn clock(&mut self, _progress: u64) -> u64 {
            panic!("these programs read no clock")
        }

        fn warn(&mut self, warning: Warning<'_>) {
            println!(
                "WebAssembly warning: {} ({})",
                warning.reason, warning.count
            );
        }
    }

    impl Side for WasmSide {
        fn protect(&mut self, at: u64, length: u64, permissions: Permissions) {
            self.cpu
                .set_permissions(self.memory, at, length, permissions)
                .expect("protected");
        }

        fn write(&mut self, at: u64, bytes: &[u8]) {
            self.cpu.write(self.memory, at, bytes).expect("written");
        }

        fn read(&mut self, at: u64, length: u64) -> Vec<u8> {
            let mut out = vec![0; length as usize];
            self.cpu.read(self.memory, at, &mut out).expect("read");
            out
        }

        fn set_registers(&mut self, block: &RegisterBlock) {
            self.cpu.set_registers(self.thread, block);
        }

        fn run(&mut self, resume: Resume) -> (Exit, RegisterBlock) {
            let mut answer = OneExit::default();
            let Ok(()) = self.cpu.run(self.thread, resume, Turn::Free, &mut answer);
            let mut block = RegisterBlock::ZERO;
            self.cpu.registers(self.thread, &mut block);
            (answer.exit.expect("an exit"), block)
        }
    }

    /// The pages of a range and the permissions of an ELF segment's flags.
    fn permissions(flags: SegmentFlags) -> Permissions {
        let SegmentFlags::Elf { p_flags } = flags else {
            return Permissions::empty();
        };
        [
            (PF_R, Permissions::READ),
            (PF_W, Permissions::WRITE),
            (PF_X, Permissions::EXECUTE),
        ]
        .into_iter()
        .filter(|(flag, _)| p_flags & flag != 0)
        .fold(Permissions::empty(), |all, (_, p)| all | p)
    }

    /// Loads a static program's segments and its stack, and sets the thread at its entry.
    fn load(image: &[u8], name: &str, stack_top: u64, side: &mut dyn Side) {
        let file = object::File::parse(image).expect("an ELF program");
        let mut pages: BTreeMap<u64, Permissions> = BTreeMap::new();
        for segment in file.segments() {
            let first = segment.address() & !(PAGE_SIZE - 1);
            let end = (segment.address() + segment.size()).next_multiple_of(PAGE_SIZE);
            for page in (first..end).step_by(PAGE_SIZE as usize) {
                *pages.entry(page).or_insert(Permissions::empty()) |= permissions(segment.flags());
            }
        }
        for (&page, &allowed) in &pages {
            side.protect(page, PAGE_SIZE, allowed);
        }
        for segment in file.segments() {
            side.write(
                segment.address(),
                segment.data().expect("the segment's bytes"),
            );
        }
        side.protect(
            stack_top - STACK_BYTES,
            STACK_BYTES,
            Permissions::READ | Permissions::WRITE,
        );
        // The start's stack as Linux lays it out: the count of arguments, the one argument (the program's name,
        // above), no environment, and an auxiliary vector with the page size alone.
        let word = size_of::<u64>() as u64;
        // Linux keeps the strings in the stack's top pages, with more above them; here a page above them is left.
        let name_at = (stack_top - PAGE_SIZE - name.len() as u64 - 1) & !(word - 1);
        let mut name_bytes = name.as_bytes().to_vec();
        name_bytes.push(0);
        side.write(name_at, &name_bytes);
        let start = [
            1,
            name_at,
            0,
            0,
            AT_PAGESZ.into(),
            PAGE_SIZE,
            AT_NULL.into(),
            0,
        ];
        let sp = (name_at - start.len() as u64 * word) & !(STACK_ALIGNMENT - 1);
        side.write(sp, start.as_bytes());
        side.set_registers(&RegisterBlock {
            pc: file.entry(),
            sp,
            ..RegisterBlock::ZERO
        });
    }

    /// Runs the program on both sides, exit by exit, answering each system call alike; gives what it wrote
    /// and how it ended.
    fn compare(
        image: &[u8],
        name: &str,
        native: &mut NativeSide,
        wasm: &mut WasmSide,
    ) -> Result<String, String> {
        let stack_top = wasm
            .cpu
            .address_space()
            .end
            .min(1 << wcontainers_arm64::mirror::REACH_BITS);
        load(image, name, stack_top, native);
        load(image, name, stack_top, wasm);
        let mut outputs: [Vec<u8>; DESCRIPTORS] = Default::default();
        let mut resume = Resume::FromRegisters;
        let mut exits = 0u64;
        loop {
            let (got, got_block) = native.run(resume);
            let (want, want_block) = wasm.run(resume);
            exits += 1;
            if got != want {
                return Err(format!(
                    "exit {exits}: native {got:?}, WebAssembly {want:?}"
                ));
            }
            if got_block != want_block {
                return Err(format!(
                    "exit {exits} ({got:?}): registers differ: {}",
                    differences(&got_block, &want_block)
                ));
            }
            let Exit::Syscall(call) = got else {
                return Ok(format!("ended by {got:?} after {exits} exits"));
            };
            let [first, second, third, ..] = call.arguments;
            let value = match call.number {
                n if n == u64::from(__NR_write) => {
                    let written = native.read(second, third);
                    if written != wasm.read(second, third) {
                        return Err(format!("exit {exits}: memory written differs"));
                    }
                    match outputs.get_mut(first as usize) {
                        Some(out) => {
                            out.extend(written);
                            third
                        }
                        None => (-i64::from(ENOSYS)) as u64,
                    }
                }
                n if n == u64::from(__NR_exit) || n == u64::from(__NR_exit_group) => {
                    return Ok(format!(
                        "status {first} after {exits} exits, wrote {:?} and {:?}",
                        String::from_utf8_lossy(&outputs[1]),
                        String::from_utf8_lossy(&outputs[2]),
                    ));
                }
                n if n == u64::from(__NR_writev) => {
                    let table = size_of::<iovec>() as u64 * third;
                    let vectors = native.read(second, table);
                    if vectors != wasm.read(second, table) {
                        return Err(format!("exit {exits}: the vectors written differ"));
                    }
                    let mut total = 0;
                    for vector in vectors.chunks(size_of::<iovec>()) {
                        let (base, length) = <[u64; 2]>::read_from_bytes(vector)
                            .map(|[base, length]| (base, length))
                            .expect("an iovec");
                        let written = native.read(base, length);
                        if written != wasm.read(base, length) {
                            return Err(format!("exit {exits}: memory written differs"));
                        }
                        if let Some(out) = outputs.get_mut(first as usize) {
                            out.extend(written);
                        }
                        total += length;
                    }
                    total
                }
                _ => (-i64::from(ENOSYS)) as u64,
            };
            resume = Resume::Return(value);
        }
    }

    /// The fields in which two register blocks differ.
    fn differences(got: &RegisterBlock, want: &RegisterBlock) -> String {
        let mut out = Vec::new();
        for (n, (a, b)) in got.x.iter().zip(&want.x).enumerate() {
            if a != b {
                out.push(format!("x{n} {a:#x} against {b:#x}"));
            }
        }
        for (n, (a, b)) in got.v.iter().zip(&want.v).enumerate() {
            if a != b {
                out.push(format!("v{n} {a:x?} against {b:x?}"));
            }
        }
        for (name, a, b) in [
            ("sp", got.sp, want.sp),
            ("pc", got.pc, want.pc),
            ("pstate", got.pstate, want.pstate),
            ("fpsr", got.fpsr.into(), want.fpsr.into()),
            ("fpcr", got.fpcr.into(), want.fpcr.into()),
            ("tpidr_el0", got.tpidr_el0, want.tpidr_el0),
        ] {
            if a != b {
                out.push(format!("{name} {a:#x} against {b:#x}"));
            }
        }
        out.join(", ")
    }
}
