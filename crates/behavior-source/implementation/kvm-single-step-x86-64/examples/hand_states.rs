//! Runs the independent check's hand-made states: those the source must refuse, each followed on the same machine
//! by a reference state that must give what a fresh machine gives, and those whose simulation disagreed with the
//! chip, each against this chip's set.
//!
//! `cargo run --release -p wcontainers-behavior-source-kvm-single-step-x86-64 --example hand_states`.

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn main() {
    on_chip::main();
}

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
fn main() {
    println!("the x86-64 chip source needs a Linux x86-64 host with KVM");
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod on_chip {
    use std::borrow::Cow;
    use std::panic::{AssertUnwindSafe, catch_unwind};

    use wcontainers_behavior_source::BehaviorSource;
    use wcontainers_behavior_source_kvm_single_step_x86_64::{
        EntryPath, MemoryLoading, Options, SingleStepMachine,
    };
    use wcontainers_distinguishing_state_generator_x86_64::CriticalValueCut;
    use wcontainers_instruction_input_domain_x86_64::X86_64InputDomain;
    use wcontainers_processor_architecture::{ExecutionState, MemoryDelta, Simulator};
    use wcontainers_processor_architecture_x86_64::{
        DataAccess, Flags, Instruction, InstructionFetch, PagePermission, Processor, X86_64, ZERO_PROCESSOR,
    };

    type State = ExecutionState<'static, X86_64>;

    const PAGE: u64 = 0x1000;
    const EXECUTABLE: PagePermission = PagePermission::Mapped {
        data_access: DataAccess::ReadOnly,
        instruction_fetch: InstructionFetch::Allowed,
    };
    const WRITABLE: PagePermission = PagePermission::Mapped {
        data_access: DataAccess::ReadWrite,
        instruction_fetch: InstructionFetch::Forbidden,
    };
    const OPTION_SETS: [(&str, Options); 3] = [
        ("full parked", Options { loading: MemoryLoading::Full, entry: EntryPath::Parked}),
        ("incremental parked", Options { loading: MemoryLoading::Incremental, entry: EntryPath::Parked}),
        ("checked parked", Options { loading: MemoryLoading::Checked, entry: EntryPath::Parked}),
    ];

    /// Code at `address` on an executable page, each listed page with its permission and a byte pattern.
    fn make(address: u64, code: &[u8], pages: &[(u64, PagePermission)], set: impl FnOnce(&mut Processor)) -> State {
        let mut processor = Processor {
            general_purpose_registers: std::array::from_fn(|number| 0x1111_0000_0000_0003 * (number as u64 + 1)),
            instruction_pointer: address,
            ..ZERO_PROCESSOR
        };
        set(&mut processor);
        let code_page = address & !(PAGE - 1);
        let mut memory = vec![MemoryDelta::PermissionAssignment {
            addresses: code_page..code_page + PAGE,
            permission: EXECUTABLE,
        }];
        for &(page, permission) in pages {
            memory.push(MemoryDelta::PermissionAssignment { addresses: page..page + PAGE, permission });
            let bytes: Vec<u8> = (page..page + PAGE).map(|address| (address as u8) ^ 0xA5).collect();
            memory.push(MemoryDelta::ValueAssignment { start: page, bytes: Cow::Owned(bytes) });
        }
        memory.push(MemoryDelta::ValueAssignment { start: address, bytes: Cow::Owned(code.to_vec()) });
        ExecutionState { processor, memory_mutations: memory }
    }

    fn run(machine: &mut SingleStepMachine, state: &State) -> Result<State, String> {
        catch_unwind(AssertUnwindSafe(|| machine.execute_single_instruction(state.clone()))).map_err(|panic| {
            panic
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| panic.downcast_ref::<&str>().map(|text| text.to_string()))
                .unwrap_or_default()
        })
    }

    fn is_same(first: &State, second: &State) -> bool {
        first.processor == second.processor && first.memory_mutations == second.memory_mutations
    }

    fn list_refused() -> Vec<(&'static str, State)> {
        let mut prefixed = vec![0x3E; 13];
        prefixed.extend([0x48, 0x01, 0xD8]);
        vec![
            (
                "the check's: maps the source's kernel stack page 0xFFFF_FFFF_FFE0_7000 and loads from it",
                make(0x1000, &[0x48, 0x8B, 0x03], &[(0xFFFF_FFFF_FFE0_7000, WRITABLE)], |processor| {
                    processor.general_purpose_registers[3] = 0xFFFF_FFFF_FFE0_7000;
                }),
            ),
            (
                "bytes past the program's addresses, at 0x8000_0000_0000",
                {
                    let mut state = make(0x1000, &[0x48, 0x01, 0xD8], &[], |_| {});
                    state.memory_mutations.push(MemoryDelta::ValueAssignment {
                        start: 0x8000_0000_0000,
                        bytes: Cow::Owned(vec![0xCC; 8]),
                    });
                    state
                },
            ),
            (
                "the page table's page 0xFFFF_FFFF_FFE0_9000 mapped writable, no access",
                make(0x1000, &[0x48, 0x01, 0xD8], &[(0xFFFF_FFFF_FFE0_9000, WRITABLE)], |_| {}),
            ),
            (
                "RFLAGS with TF and RF set",
                make(0x1000, &[0x48, 0x01, 0xD8], &[], |processor| {
                    processor.flags = Flags::from_bits_retain(1 << 8 | 1 << 16 | 1);
                }),
            ),
            ("MOV SS, AX then ADD", make(0x1000, &[0x8E, 0xD0, 0x48, 0x01, 0xD8], &[], |_| {})),
            (
                "REP STOSB",
                make(0x1000, &[0xF3, 0xAA], &[(0x2000, WRITABLE)], |processor| {
                    processor.general_purpose_registers[1] = 3;
                    processor.general_purpose_registers[7] = 0x2000;
                }),
            ),
            ("POPFQ", make(0x1000, &[0x9D], &[(0x2000, WRITABLE)], |processor| processor.general_purpose_registers[4] = 0x2800)),
            ("PUSHFQ", make(0x1000, &[0x9C], &[(0x2000, WRITABLE)], |processor| processor.general_purpose_registers[4] = 0x2800)),
            ("SYSCALL", make(0x1000, &[0x0F, 0x05], &[], |_| {})),
            ("far RET", make(0x1000, &[0x48, 0xCB], &[(0x2000, WRITABLE)], |processor| processor.general_purpose_registers[4] = 0x2800)),
            ("MOV FS, AX", make(0x1000, &[0x8E, 0xE0], &[], |_| {})),
            ("VMCALL", make(0x1000, &[0x0F, 0x01, 0xC1], &[], |_| {})),
            ("SYSENTER", make(0x1000, &[0x0F, 0x34], &[], |_| {})),
            ("EVEX VPADDD", make(0x1000, &EVEX_ADD, &[], |_| {})),
            ("15 bytes, DS 9 times then EVEX VPADDD", make(0x1000, &padded(0x3E, 9, &EVEX_ADD), &[], |_| {})),
            ("RDTSC", make(0x1000, &[0x0F, 0x31], &[], |_| {})),
            ("RDTSCP", make(0x1000, &[0x0F, 0x01, 0xF9], &[], |_| {})),
            ("RDPID RAX", make(0x1000, &[0xF3, 0x0F, 0xC7, 0xF8], &[], |_| {})),
            ("RDPKRU", make(0x1000, &[0x0F, 0x01, 0xEE], &[], |processor| processor.general_purpose_registers[1] = 0)),
            ("XGETBV", make(0x1000, &[0x0F, 0x01, 0xD0], &[], |processor| processor.general_purpose_registers[1] = 0)),
            ("XSAVE [RBX]", make(0x1000, &[0x0F, 0xAE, 0x23], &[(0x2000, WRITABLE)], |processor| processor.general_purpose_registers[3] = 0x2000)),
            ("LAR EAX, EBX", make(0x1000, &[0x0F, 0x02, 0xC3], &[], |processor| processor.general_purpose_registers[3] = 0x33)),
            ("LSL EAX, EBX", make(0x1000, &[0x0F, 0x03, 0xC3], &[], |processor| processor.general_purpose_registers[3] = 0x7B)),
            ("VERR BX", make(0x1000, &[0x0F, 0x00, 0xE3], &[], |processor| processor.general_purpose_registers[3] = 0x2B)),
            ("SGDT [RBX]", make(0x1000, &[0x0F, 0x01, 0x03], &[(0x2000, WRITABLE)], |processor| processor.general_purpose_registers[3] = 0x2000)),
            ("SMSW EAX", make(0x1000, &[0x0F, 0x01, 0xE0], &[], |_| {})),
            ("INT 0x80", make(0x1000, &[0xCD, 0x80], &[], |_| {})),
            ("LDTILECFG [RBX]", make(0x1000, &[0xC4, 0xE2, 0x78, 0x49, 0x03], &[(0x2000, WRITABLE)], |processor| processor.general_purpose_registers[3] = 0x2000)),
            ("too many pages for the arena", {
                let mut state = make(0x1000, &[0x48, 0x01, 0xD8], &[], |_| {});
                state.memory_mutations.push(MemoryDelta::PermissionAssignment {
                    addresses: 0x10_0000..0x10_0000 + 4096 * PAGE,
                    permission: WRITABLE,
                });
                state
            }),
        ]
    }

    fn list_disagreeing() -> Vec<(&'static str, Instruction, State)> {
        let mut prefixed_16 = vec![0x3E; 13];
        prefixed_16.extend([0x48, 0x01, 0xD8]);
        vec![
            ("16-byte ADD", Instruction::AddGprvGprv01, make(0x1000, &prefixed_16, &[], |_| {})),
            ("16-byte ADD, 16th byte on an unmapped page", Instruction::AddGprvGprv01, make(0x1FF1, &prefixed_16, &[], |_| {})),
            (
                "MOV RAX, FS:[RBP], RBP 1<<63",
                Instruction::MovGprvMemv,
                make(0x1000, &[0x64, 0x48, 0x8B, 0x45, 0x00], &[], |processor| {
                    processor.general_purpose_registers[5] = 1 << 63;
                }),
            ),
            ("ADD at 0x7FFF_FFFF_FFFE, across the canonical edge", Instruction::AddGprvGprv01, make(0x7FFF_FFFF_FFFE, &[0x48, 0x01], &[], |_| {})),
            ("ADD ending at the last canonical address", Instruction::AddGprvGprv01, make(0x7FFF_FFFF_FFFD, &[0x48, 0x01, 0xD8], &[], |_| {})),
            (
                "AC, MOV RAX, [RBX] at 0x2001",
                Instruction::MovGprvMemv,
                make(0x1000, &[0x48, 0x8B, 0x03], &[(0x2000, WRITABLE)], |processor| {
                    processor.general_purpose_registers[3] = 0x2001;
                    processor.flags = Flags::ALIGNMENT_CHECK;
                }),
            ),
            (
                "AC, PUSH RAX, RSP 0x3003",
                Instruction::PushGprv50,
                make(0x1000, &[0x50], &[(0x2000, WRITABLE)], |processor| {
                    processor.general_purpose_registers[4] = 0x3003;
                    processor.flags = Flags::ALIGNMENT_CHECK;
                }),
            ),
            (
                "MOV RAX, [RBX] from the source's kernel page 0xFFFF_FFFF_FFE0_5000",
                Instruction::MovGprvMemv,
                make(0x1000, &[0x48, 0x8B, 0x03], &[], |processor| {
                    processor.general_purpose_registers[3] = 0xFFFF_FFFF_FFE0_5000;
                }),
            ),
            (
                "MOV RAX, [RBX] from 0xFFFF_8000_0000_0000",
                Instruction::MovGprvMemv,
                make(0x1000, &[0x48, 0x8B, 0x03], &[], |processor| {
                    processor.general_purpose_registers[3] = 0xFFFF_8000_0000_0000;
                }),
            ),
            (
                "the second check's: MOV RAX, [EIP+0xFF8], RCX 6",
                Instruction::MovGprvMemv,
                make(0x1000, &[0x67, 0x48, 0x8B, 0x05, 0xF8, 0x0F, 0x00, 0x00], &[(0x2000, WRITABLE)], |processor| {
                    processor.general_purpose_registers[1] = 6;
                }),
            ),
            (
                "AC, MOV RAX, [RBX], RBX 0x8000_0000_0001",
                Instruction::MovGprvMemv,
                make(0x1000, &[0x48, 0x8B, 0x03], &[], |processor| {
                    processor.general_purpose_registers[3] = 0x8000_0000_0001;
                    processor.flags = Flags::ALIGNMENT_CHECK;
                }),
            ),
            (
                "AC, MOV RAX, [RBP], RBP 0x8000_0000_0001",
                Instruction::MovGprvMemv,
                make(0x1000, &[0x48, 0x8B, 0x45, 0x00], &[], |processor| {
                    processor.general_purpose_registers[5] = 0x8000_0000_0001;
                    processor.flags = Flags::ALIGNMENT_CHECK;
                }),
            ),
            (
                "16 bytes, no prefix repeated: LOCK AND [R8+R11*2+disp32], imm32 under SS, DS, 0x67",
                Instruction::AndLockMemvImmz,
                make(
                    0x1000,
                    &[0x36, 0x3E, 0xF0, 0x67, 0x4A, 0x81, 0xA4, 0x58, 0x15, 0xC9, 0x63, 0x73, 0x80, 0x00, 0x00, 0x00],
                    &[],
                    |_| {},
                ),
            ),
            ("16 bytes, LOCK 13 times then ADD", Instruction::AddGprvGprv01, {
                let mut locked = vec![0xF0; 13];
                locked.extend([0x48, 0x01, 0xD8]);
                make(0x1000, &locked, &[], |_| {})
            }),
            (
                "LOCK and TEST's opcode at 0x7FFF_FFFF_FFFE, its ModRM past the canonical edge",
                Instruction::TestMembImmbF6r0,
                make(0x7FFF_FFFF_FFFE, &[0xF0, 0xF6], &[], |_| {}),
            ),
            (
                "LOCK and TEST's opcode at the end of a page whose next page is unmapped",
                Instruction::TestMembImmbF6r0,
                make(0x1FFE, &[0xF0, 0xF6, 0x03, 0x00], &[], |_| {}),
            ),
            ("SYSRET", Instruction::Sysret, make(0x1000, &[0x0F, 0x07], &[], |_| {})),
            ("SYSRETQ", Instruction::Sysret64, make(0x1000, &[0x48, 0x0F, 0x07], &[], |_| {})),
            ("SYSEXIT", Instruction::Sysexit, make(0x1000, &[0x0F, 0x35], &[], |_| {})),
            ("SWAPGS", Instruction::Swapgs, make(0x1000, &[0x0F, 0x01, 0xF8], &[], |_| {})),
            ("the third check's: 16 bytes, REX.W 14 times then ADD", Instruction::AddGprvGprv01, make(0x1000, &padded(0x48, 14, &[0x01, 0xD8]), &[], |_| {})),
            ("15 bytes, REX.W 13 times then ADD, the last REX counting", Instruction::AddGprvGprv01, make(0x1000, &padded(0x48, 13, &[0x01, 0xD8]), &[], |_| {})),
            ("16 bytes, REX.W 12 times, DS, then REX.W ADD", Instruction::AddGprvGprv01, make(0x1000, &padded(0x48, 12, &[0x3E, 0x48, 0x01, 0xD8]), &[], |_| {})),
            ("16 bytes, REX 13 times alternating, then ADD", Instruction::AddGprvGprv01, make(0x1000, &[0x40, 0x4F, 0x40, 0x4F, 0x40, 0x4F, 0x40, 0x4F, 0x40, 0x4F, 0x40, 0x4F, 0x40, 0x48, 0x01, 0xD8], &[], |_| {})),
            ("16 bytes, DS 12 times then VEX VPADDD", Instruction::AddGprvGprv01, make(0x1000, &padded(0x3E, 12, &VEX_ADD), &[], |_| {})),
            ("16 bytes, REX 12 times then VEX VPADDD", Instruction::AddGprvGprv01, make(0x1000, &padded(0x48, 12, &VEX_ADD), &[], |_| {})),
            ("16 bytes, DS 11 times, 0x66, then VEX VPADDD", Instruction::AddGprvGprv01, make(0x1000, &padded(0x3E, 11, &[&[0x66][..], &VEX_ADD].concat()), &[], |_| {})),
            ("16 bytes, DS 10 times then EVEX VPADDD", Instruction::AddGprvGprv01, make(0x1000, &padded(0x3E, 10, &EVEX_ADD), &[], |_| {})),
            ("16 bytes, DS 10 times then EVEX VPADDD, the 16th byte on an unmapped page", Instruction::AddGprvGprv01, make(0x1FF1, &padded(0x3E, 10, &EVEX_ADD), &[], |_| {})),
            // XCR0 has no AVX state and CR4.OSXSAVE is clear: #UD once measured, whatever form the behavior names.
            ("15 bytes, REX 11 times then VEX VPADDD", Instruction::AddGprvGprv01, make(0x1000, &padded(0x48, 11, &VEX_ADD), &[], |_| {})),
        ]
    }

    /// VPADDD XMM0, XMM0, XMM1 with a two-byte VEX prefix, and with EVEX.
    const VEX_ADD: [u8; 4] = [0xC5, 0xF9, 0xFE, 0xC1];
    const EVEX_ADD: [u8; 6] = [0x62, 0xF1, 0x7D, 0x08, 0xFE, 0xC1];

    /// The prefix `count` times, then the bytes.
    fn padded(prefix: u8, count: usize, bytes: &[u8]) -> Vec<u8> {
        let mut all = vec![prefix; count];
        all.extend_from_slice(bytes);
        all
    }

    pub fn main() {
        let reference = make(0x1000, &[0x48, 0x01, 0x03], &[(0x2000, WRITABLE)], |processor| {
            processor.general_purpose_registers[3] = 0x2008;
        });
        let fresh = SingleStepMachine::with_options(OPTION_SETS[0].1)
            .unwrap_or_else(|error| panic!("{error:?}"))
            .execute_single_instruction(reference.clone());
        for (name, state) in list_refused() {
            println!("-- refused? {name}");
            for (label, options) in OPTION_SETS {
                let mut machine = SingleStepMachine::with_options(options).unwrap_or_else(|error| panic!("{error:?}"));
                // A state first, so that the machine holds something the refused state could overwrite.
                let first = run(&mut machine, &reference).map(|after| is_same(&after, &fresh));
                let outcome = match run(&mut machine, &state) {
                    Ok(_) => "RAN".to_string(),
                    Err(message) => format!("refused: {message}"),
                };
                let next = match run(&mut machine, &reference) {
                    Ok(after) if is_same(&after, &fresh) => "the next state as on a fresh machine".to_string(),
                    Ok(_) => "the next state DIFFERS".to_string(),
                    Err(message) => format!("the next state PANICS: {message}"),
                };
                println!("   {label}: first {first:?}; {outcome}; {next}");
            }
        }
        let mut machine = SingleStepMachine::with_options(OPTION_SETS[0].1).unwrap_or_else(|error| panic!("{error:?}"));
        for (name, instruction, state) in list_disagreeing() {
            // This chip's behavior of the form, as the provided measuring loop finds it.
            let generator = CriticalValueCut::new(X86_64InputDomain);
            let Ok(chosen) = machine.measure_instruction(instruction, &generator) else {
                panic!("{instruction:?} shows no one known behavior on this chip");
            };
            let executed = run(&mut machine, &state);
            let Ok(simulated) = chosen.simulate(state.clone());
            match executed {
                Ok(executed) => {
                    let agrees = simulated == executed;
                    println!(
                        "-- {name}: chip {:?} ip {:#x}; simulation {:?} ip {:#x}: {}",
                        executed.processor.exception,
                        executed.processor.instruction_pointer,
                        simulated.processor.exception,
                        simulated.processor.instruction_pointer,
                        if agrees { "AGREES" } else { "DISAGREES" }
                    );
                }
                Err(message) => println!("-- {name}: PANIC {message}"),
            }
        }
    }
}
