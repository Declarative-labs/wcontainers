use std::borrow::Cow;
use std::convert::Infallible;

use wcontainers_abi::{LOWEST_ADDRESS, PAGE_SIZE};
use wcontainers_behavior_source::BehaviorSource;
use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_distinguishing_state_generator_arm64::CriticalValueCut;
use wcontainers_executor_conformance::AGREEMENT_EXECUTORS;
use wcontainers_executor_unicorn::standin::StandIn;
use wcontainers_precise_instruction_set::PreciseInstructionSet;
use wcontainers_precise_instruction_set_measured::MeasuredInstructionSet;
use wcontainers_processor_architecture::{
    ExecutionState, MemoryDelta, ProcessorArchitecture, Simulator,
};
use wcontainers_processor_architecture_arm64::{
    Arm64, DataAccess, Instruction, InstructionFetch, PagePermission, Processor, User,
    ZERO_PROCESSOR,
};

/// Returns for every instruction the first known behavior that each state the stand-in executes meets, or its first
/// known behavior where none does.
///
/// Where the architecture knows several behaviors of an instruction, the chip picks one, and the stand-in is the chip
/// QEMU's `max` model is: the agreement test then fails exactly where no known behavior explains what it does.
///
/// The instructions are shared out among as many stand-ins as the agreement test runs.
fn list_stand_in_behaviors() -> MeasuredInstructionSet<Arm64<User>> {
    let instructions: Vec<_> = Arm64::<User>::list_instructions().collect();
    let mut measurements: Vec<_> = std::thread::scope(|scope| {
        let runs: Vec<_> = (0..AGREEMENT_EXECUTORS)
            .map(|share| {
                let instructions = &instructions;
                scope.spawn(move || {
                    let generator = CriticalValueCut::default();
                    let mut stand_in = StandIn::new();
                    instructions
                        .iter()
                        .enumerate()
                        .skip(share)
                        .step_by(AGREEMENT_EXECUTORS)
                        .filter_map(|(index, instruction)| {
                            let shown =
                                find_shown_behavior(&mut stand_in, &generator, *instruction)?;
                            Some((index, (*instruction, shown)))
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        runs.into_iter()
            .flat_map(|run| run.join().expect("a measuring run ends"))
            .collect()
    });
    measurements.sort_by_key(|(index, _)| *index);
    let measurements: Vec<_> = measurements
        .into_iter()
        .map(|(_, measurement)| measurement)
        .collect();
    MeasuredInstructionSet::from_measurements(&measurements)
}

/// Returns the first known behavior of the instruction every state the stand-in executes meets, or its first known
/// behavior where none does; none where it has no known behavior.
fn find_shown_behavior(
    stand_in: &mut StandIn,
    generator: &CriticalValueCut,
    instruction: <Arm64<User> as ProcessorArchitecture>::Instruction,
) -> Option<<Arm64<User> as ProcessorArchitecture>::PreciseInstruction> {
    let known: Vec<_> = Arm64::<User>::list_known_behaviors(instruction).collect();
    let first = *known.first()?;
    let executed: Vec<_> = generator
        .generate_states(instruction)
        .filter_map(|before| {
            let after = stand_in.execute_single_instruction(before.clone()).ok()?;
            Some((before, after))
        })
        .collect();
    let shown = known.into_iter().find(|precise| {
        executed.iter().all(|(before, after)| {
            let Ok(expected) = precise.simulate_state(before.clone());
            after.meets_expectations(&expected)
        })
    });
    Some(shown.unwrap_or(first))
}

/// An unallocated Advanced SIMD three same (FP16) word, on which QEMU's decoder asserts.
const EMULATOR_ABORTING_WORD: u32 = 0x0EC0_1DCD;
/// MRS X0, DAIF, which EL1 traps at EL0 and unicorn stops on as undefined.
const TRAPPED_WORD: u32 = 0xD53B_4220;

/// The states the stand-in refuses: one whose instruction would end the host process, one whose stop it cannot tell.
fn list_unexecutable_states() -> Vec<ExecutionState<'static, Arm64<User>>> {
    [EMULATOR_ABORTING_WORD, TRAPPED_WORD]
        .map(|word| ExecutionState {
            processor: Processor {
                program_counter: LOWEST_ADDRESS,
                ..ZERO_PROCESSOR
            },
            memory_mutations: vec![
                MemoryDelta::PermissionAssignment {
                    addresses: LOWEST_ADDRESS..LOWEST_ADDRESS + PAGE_SIZE,
                    permission: PagePermission::Mapped {
                        data_access: DataAccess::ReadOnly,
                        instruction_fetch: InstructionFetch::Allowed,
                    },
                },
                MemoryDelta::ValueAssignment {
                    start: LOWEST_ADDRESS,
                    bytes: Cow::Owned(word.to_le_bytes().to_vec()),
                },
            ],
        })
        .into()
}

/// A vector load that faults leaves part of what it loaded in its registers, which the chip leaves unchanged.
const VECTOR_LOADS_ACROSS_FAULT: &str = "MEASURED 2026-11-26 on a Neoverse V2 (AWS Graviton4, KVM) by the measured set's default_behaviors, the model agreeing on every generated state: a vector load that faults leaves part of what it loaded in its registers, which the chip leaves unchanged";
/// A contiguous first-fault load leaves FFR and its elements other than the chip does.
const FIRST_FAULT_LOADS: &str = "MEASURED 2026-11-26 on a Neoverse V2 (AWS Graviton4, KVM) by the measured set's default_behaviors, the model agreeing on every generated state: a contiguous first-fault load leaves FFR and its elements other than the chip does";
/// An SVE store that faults leaves memory other than the chip does.
const SVE_STORES_ACROSS_FAULT: &str = "MEASURED 2026-11-26 on a Neoverse V2 (AWS Graviton4, KVM) by the measured set's default_behaviors, the model agreeing on every generated state: an SVE store that faults leaves memory other than the chip does";
/// XPACI, XPACD and XPACLRI keep a top byte the chip replaces with copies of bit 55.
const POINTER_STRIP: &str = "MEASURED 2026-11-26 on a Neoverse V2 (AWS Graviton4, KVM) by the measured set's default_behaviors, the model agreeing on every generated state: XPACI, XPACD and XPACLRI keep a top byte the chip replaces with copies of bit 55";
/// SYS completes on encodings the chip traps.
const SYSTEM_INSTRUCTION: &str = "MEASURED 2026-11-26 on a Neoverse V2 (AWS Graviton4, KVM) by the measured set's default_behaviors, the model agreeing on every generated state: SYS completes on encodings the chip traps";
/// MSR FPCR keeps bits 0x07FF0000 where the chip keeps 0x07C80000, and MRS CTR_EL0 reads 0x8444C004 where the chip reads 0xB444C004.
const SYSTEM_REGISTERS: &str = "MEASURED 2026-11-26 on a Neoverse V2 (AWS Graviton4, KVM) by the measured set's default_behaviors, the model agreeing on every generated state: MSR FPCR keeps bits 0x07FF0000 where the chip keeps 0x07C80000, and MRS CTR_EL0 reads 0x8444C004 where the chip reads 0xB444C004";
/// A misaligned atomic or compare-and-swap raises an alignment fault where the chip raises a translation fault.
const ATOMIC_ALIGNMENT: &str = "MEASURED 2026-11-26 on a Neoverse V2 (AWS Graviton4, KVM) by the measured set's default_behaviors, the model agreeing on every generated state: a misaligned atomic or compare-and-swap raises an alignment fault where the chip raises a translation fault";
/// A store exclusive to a misaligned address completes, failing the store, where the chip raises an alignment fault.
const EXCLUSIVE_STORE_ALIGNMENT: &str = "MEASURED 2026-11-26 on a Neoverse V2 (AWS Graviton4, KVM) by the measured set's default_behaviors, the model agreeing on every generated state: a store exclusive to a misaligned address completes, failing the store, where the chip raises an alignment fault";
/// A load exclusive meets different known behaviors on different states, where the chip keeps one.
const EXCLUSIVE_LOAD_BEHAVIORS: &str = "MEASURED 2026-11-26 on a Neoverse V2 (AWS Graviton4, KVM) by the measured set's default_behaviors, the model agreeing on every generated state: a load exclusive meets different known behaviors on different states, where the chip keeps one";
/// A misaligned load-acquire or store-release raises a translation fault where the chip raises an alignment fault.
const ORDERED_ALIGNMENT: &str = "MEASURED 2026-11-26 on a Neoverse V2 (AWS Graviton4, KVM) by the measured set's default_behaviors, the model agreeing on every generated state: a misaligned load-acquire or store-release raises a translation fault where the chip raises an alignment fault";
/// A signed byte maximum or minimum stores a different byte from the chip.
const SIGNED_BYTE_ATOMICS: &str = "MEASURED 2026-11-26 on a Neoverse V2 (AWS Graviton4, KVM) by the measured set's default_behaviors, the model agreeing on every generated state: a signed byte maximum or minimum stores a different byte from the chip";
/// FCVTZS (scalar, fixed-point, half) writes 32 bits, FCMLA (by element) raises overflow and FJCVTZS sets Z on states where the chip does not.
const FLOATING_POINT: &str = "MEASURED 2026-11-26 on a Neoverse V2 (AWS Graviton4, KVM) by the measured set's default_behaviors, the model agreeing on every generated state: FCVTZS (scalar, fixed-point, half) writes 32 bits, FCMLA (by element) raises overflow and FJCVTZS sets Z on states where the chip does not";
/// QEMU runs the SM3 and SM4 instructions, which the chip lacks: it raises an undefined instruction.
const ABSENT_FEATURES: &str = "MEASURED 2026-11-26 on a Neoverse V2 (AWS Graviton4, KVM) by the measured set's default_behaviors, the model agreeing on every generated state: QEMU runs the SM3 and SM4 instructions, which the chip lacks: it raises an undefined instruction";

/// How many instructions are set aside: one more is a change of this count, never a silent one.
const SET_ASIDE_COUNT: usize = 353;

/// The instructions where QEMU's `max` model differs from the chip, each with the measurement that shows it, set aside
/// from the agreement test; the list of known behaviors holds real chips' behaviors only.
const SET_ASIDE: [(Instruction<Infallible>, &str); SET_ASIDE_COUNT] = [
    (Instruction::LdrZXI, VECTOR_LOADS_ACROSS_FAULT),
    (Instruction::Ld4Four, VECTOR_LOADS_ACROSS_FAULT),
    (Instruction::Ld1Four, VECTOR_LOADS_ACROSS_FAULT),
    (Instruction::Ld3Three, VECTOR_LOADS_ACROSS_FAULT),
    (Instruction::Ld1Three, VECTOR_LOADS_ACROSS_FAULT),
    (Instruction::Ld1One, VECTOR_LOADS_ACROSS_FAULT),
    (Instruction::Ld2Two, VECTOR_LOADS_ACROSS_FAULT),
    (Instruction::Ld1Two, VECTOR_LOADS_ACROSS_FAULT),
    (
        Instruction::Ld4FourRegisterPostIndex,
        VECTOR_LOADS_ACROSS_FAULT,
    ),
    (
        Instruction::Ld1FourRegisterPostIndex,
        VECTOR_LOADS_ACROSS_FAULT,
    ),
    (
        Instruction::Ld3ThreeRegisterPostIndex,
        VECTOR_LOADS_ACROSS_FAULT,
    ),
    (
        Instruction::Ld1ThreeRegisterPostIndex,
        VECTOR_LOADS_ACROSS_FAULT,
    ),
    (
        Instruction::Ld1OneRegisterPostIndex,
        VECTOR_LOADS_ACROSS_FAULT,
    ),
    (
        Instruction::Ld2TwoRegisterPostIndex,
        VECTOR_LOADS_ACROSS_FAULT,
    ),
    (
        Instruction::Ld1TwoRegisterPostIndex,
        VECTOR_LOADS_ACROSS_FAULT,
    ),
    (
        Instruction::Ld4FourImmediatePostIndex,
        VECTOR_LOADS_ACROSS_FAULT,
    ),
    (
        Instruction::Ld1FourImmediatePostIndex,
        VECTOR_LOADS_ACROSS_FAULT,
    ),
    (
        Instruction::Ld3ThreeImmediatePostIndex,
        VECTOR_LOADS_ACROSS_FAULT,
    ),
    (
        Instruction::Ld1ThreeImmediatePostIndex,
        VECTOR_LOADS_ACROSS_FAULT,
    ),
    (
        Instruction::Ld1OneImmediatePostIndex,
        VECTOR_LOADS_ACROSS_FAULT,
    ),
    (
        Instruction::Ld2TwoImmediatePostIndex,
        VECTOR_LOADS_ACROSS_FAULT,
    ),
    (
        Instruction::Ld1TwoImmediatePostIndex,
        VECTOR_LOADS_ACROSS_FAULT,
    ),
    (Instruction::Ld3I8, VECTOR_LOADS_ACROSS_FAULT),
    (Instruction::Ld3I16, VECTOR_LOADS_ACROSS_FAULT),
    (Instruction::Ld3I32, VECTOR_LOADS_ACROSS_FAULT),
    (Instruction::Ld3I64, VECTOR_LOADS_ACROSS_FAULT),
    (Instruction::Ld3r, VECTOR_LOADS_ACROSS_FAULT),
    (Instruction::Ld2I8, VECTOR_LOADS_ACROSS_FAULT),
    (Instruction::Ld4I8, VECTOR_LOADS_ACROSS_FAULT),
    (Instruction::Ld2I16, VECTOR_LOADS_ACROSS_FAULT),
    (Instruction::Ld4I16, VECTOR_LOADS_ACROSS_FAULT),
    (Instruction::Ld2I32, VECTOR_LOADS_ACROSS_FAULT),
    (Instruction::Ld2I64, VECTOR_LOADS_ACROSS_FAULT),
    (Instruction::Ld4I32, VECTOR_LOADS_ACROSS_FAULT),
    (Instruction::Ld4I64, VECTOR_LOADS_ACROSS_FAULT),
    (Instruction::Ld2r, VECTOR_LOADS_ACROSS_FAULT),
    (Instruction::Ld4r, VECTOR_LOADS_ACROSS_FAULT),
    (
        Instruction::Ld3I8RegisterPostIndex,
        VECTOR_LOADS_ACROSS_FAULT,
    ),
    (
        Instruction::Ld3I16RegisterPostIndex,
        VECTOR_LOADS_ACROSS_FAULT,
    ),
    (
        Instruction::Ld3I32RegisterPostIndex,
        VECTOR_LOADS_ACROSS_FAULT,
    ),
    (
        Instruction::Ld3I64RegisterPostIndex,
        VECTOR_LOADS_ACROSS_FAULT,
    ),
    (
        Instruction::Ld3rRegisterPostIndex,
        VECTOR_LOADS_ACROSS_FAULT,
    ),
    (
        Instruction::Ld3I8ImmediatePostIndex,
        VECTOR_LOADS_ACROSS_FAULT,
    ),
    (
        Instruction::Ld3I16ImmediatePostIndex,
        VECTOR_LOADS_ACROSS_FAULT,
    ),
    (
        Instruction::Ld3I32ImmediatePostIndex,
        VECTOR_LOADS_ACROSS_FAULT,
    ),
    (
        Instruction::Ld3I64ImmediatePostIndex,
        VECTOR_LOADS_ACROSS_FAULT,
    ),
    (
        Instruction::Ld3rImmediatePostIndex,
        VECTOR_LOADS_ACROSS_FAULT,
    ),
    (
        Instruction::Ld2I8RegisterPostIndex,
        VECTOR_LOADS_ACROSS_FAULT,
    ),
    (
        Instruction::Ld4I8RegisterPostIndex,
        VECTOR_LOADS_ACROSS_FAULT,
    ),
    (
        Instruction::Ld2I16RegisterPostIndex,
        VECTOR_LOADS_ACROSS_FAULT,
    ),
    (
        Instruction::Ld4I16RegisterPostIndex,
        VECTOR_LOADS_ACROSS_FAULT,
    ),
    (
        Instruction::Ld2I32RegisterPostIndex,
        VECTOR_LOADS_ACROSS_FAULT,
    ),
    (
        Instruction::Ld2I64RegisterPostIndex,
        VECTOR_LOADS_ACROSS_FAULT,
    ),
    (
        Instruction::Ld4I32RegisterPostIndex,
        VECTOR_LOADS_ACROSS_FAULT,
    ),
    (
        Instruction::Ld4I64RegisterPostIndex,
        VECTOR_LOADS_ACROSS_FAULT,
    ),
    (
        Instruction::Ld2rRegisterPostIndex,
        VECTOR_LOADS_ACROSS_FAULT,
    ),
    (
        Instruction::Ld4rRegisterPostIndex,
        VECTOR_LOADS_ACROSS_FAULT,
    ),
    (
        Instruction::Ld2I8ImmediatePostIndex,
        VECTOR_LOADS_ACROSS_FAULT,
    ),
    (
        Instruction::Ld4I8ImmediatePostIndex,
        VECTOR_LOADS_ACROSS_FAULT,
    ),
    (
        Instruction::Ld2I16ImmediatePostIndex,
        VECTOR_LOADS_ACROSS_FAULT,
    ),
    (
        Instruction::Ld4I16ImmediatePostIndex,
        VECTOR_LOADS_ACROSS_FAULT,
    ),
    (
        Instruction::Ld2I32ImmediatePostIndex,
        VECTOR_LOADS_ACROSS_FAULT,
    ),
    (
        Instruction::Ld2I64ImmediatePostIndex,
        VECTOR_LOADS_ACROSS_FAULT,
    ),
    (
        Instruction::Ld4I32ImmediatePostIndex,
        VECTOR_LOADS_ACROSS_FAULT,
    ),
    (
        Instruction::Ld4I64ImmediatePostIndex,
        VECTOR_LOADS_ACROSS_FAULT,
    ),
    (
        Instruction::Ld2rImmediatePostIndex,
        VECTOR_LOADS_ACROSS_FAULT,
    ),
    (
        Instruction::Ld4rImmediatePostIndex,
        VECTOR_LOADS_ACROSS_FAULT,
    ),
    (
        Instruction::LdnpSimdFpSignedOffset32,
        VECTOR_LOADS_ACROSS_FAULT,
    ),
    (
        Instruction::LdnpSimdFpSignedOffset64,
        VECTOR_LOADS_ACROSS_FAULT,
    ),
    (
        Instruction::LdnpSimdFpSignedOffset128,
        VECTOR_LOADS_ACROSS_FAULT,
    ),
    (Instruction::LdpSimdFpPostIndex32, VECTOR_LOADS_ACROSS_FAULT),
    (Instruction::LdpSimdFpPostIndex64, VECTOR_LOADS_ACROSS_FAULT),
    (
        Instruction::LdpSimdFpPostIndex128,
        VECTOR_LOADS_ACROSS_FAULT,
    ),
    (
        Instruction::LdpSimdFpSignedOffset32,
        VECTOR_LOADS_ACROSS_FAULT,
    ),
    (
        Instruction::LdpSimdFpSignedOffset64,
        VECTOR_LOADS_ACROSS_FAULT,
    ),
    (
        Instruction::LdpSimdFpSignedOffset128,
        VECTOR_LOADS_ACROSS_FAULT,
    ),
    (Instruction::LdpSimdFpPreIndex32, VECTOR_LOADS_ACROSS_FAULT),
    (Instruction::LdpSimdFpPreIndex64, VECTOR_LOADS_ACROSS_FAULT),
    (Instruction::LdpSimdFpPreIndex128, VECTOR_LOADS_ACROSS_FAULT),
    (Instruction::Ldff1b, FIRST_FAULT_LOADS),
    (Instruction::Ldff1b16, FIRST_FAULT_LOADS),
    (Instruction::Ldff1b32, FIRST_FAULT_LOADS),
    (Instruction::Ldff1b64, FIRST_FAULT_LOADS),
    (Instruction::Ldff1sw64, FIRST_FAULT_LOADS),
    (Instruction::Ldff1h, FIRST_FAULT_LOADS),
    (Instruction::Ldff1h32, FIRST_FAULT_LOADS),
    (Instruction::Ldff1h64, FIRST_FAULT_LOADS),
    (Instruction::Ldff1sh64, FIRST_FAULT_LOADS),
    (Instruction::Ldff1sh32, FIRST_FAULT_LOADS),
    (Instruction::Ldff1w, FIRST_FAULT_LOADS),
    (Instruction::Ldff1w64, FIRST_FAULT_LOADS),
    (Instruction::Ldff1sb64, FIRST_FAULT_LOADS),
    (Instruction::Ldff1sb32, FIRST_FAULT_LOADS),
    (Instruction::Ldff1sb16, FIRST_FAULT_LOADS),
    (Instruction::Ldff1d, FIRST_FAULT_LOADS),
    (Instruction::St1bZPBr, SVE_STORES_ACROSS_FAULT),
    (Instruction::St1hZPBr, SVE_STORES_ACROSS_FAULT),
    (Instruction::St1wZPBr, SVE_STORES_ACROSS_FAULT),
    (Instruction::St1d, SVE_STORES_ACROSS_FAULT),
    (Instruction::Stnt1bZRR, SVE_STORES_ACROSS_FAULT),
    (Instruction::Stnt1hZRR, SVE_STORES_ACROSS_FAULT),
    (Instruction::Stnt1wZRR, SVE_STORES_ACROSS_FAULT),
    (Instruction::Stnt1dZRR, SVE_STORES_ACROSS_FAULT),
    (Instruction::St2b, SVE_STORES_ACROSS_FAULT),
    (Instruction::St3b, SVE_STORES_ACROSS_FAULT),
    (Instruction::St4b, SVE_STORES_ACROSS_FAULT),
    (Instruction::St2h, SVE_STORES_ACROSS_FAULT),
    (Instruction::St3h, SVE_STORES_ACROSS_FAULT),
    (Instruction::St4h, SVE_STORES_ACROSS_FAULT),
    (Instruction::St2w, SVE_STORES_ACROSS_FAULT),
    (Instruction::St3w, SVE_STORES_ACROSS_FAULT),
    (Instruction::St4w, SVE_STORES_ACROSS_FAULT),
    (Instruction::St2d, SVE_STORES_ACROSS_FAULT),
    (Instruction::St3d, SVE_STORES_ACROSS_FAULT),
    (Instruction::St4d, SVE_STORES_ACROSS_FAULT),
    (Instruction::St1bZPBzDX32Unscaled, SVE_STORES_ACROSS_FAULT),
    (Instruction::St1hZPBzDX32Unscaled, SVE_STORES_ACROSS_FAULT),
    (Instruction::St1wZPBzDX32Unscaled, SVE_STORES_ACROSS_FAULT),
    (Instruction::St1dZPBzDX32Unscaled, SVE_STORES_ACROSS_FAULT),
    (Instruction::St1bZPBzSX32Unscaled, SVE_STORES_ACROSS_FAULT),
    (Instruction::St1hZPBzSX32Unscaled, SVE_STORES_ACROSS_FAULT),
    (Instruction::St1wZPBzSX32Unscaled, SVE_STORES_ACROSS_FAULT),
    (Instruction::St1hZPBzDX32Scaled, SVE_STORES_ACROSS_FAULT),
    (Instruction::St1wZPBzDX32Scaled, SVE_STORES_ACROSS_FAULT),
    (Instruction::St1dZPBzDX32Scaled, SVE_STORES_ACROSS_FAULT),
    (Instruction::St1hZPBzSX32Scaled, SVE_STORES_ACROSS_FAULT),
    (Instruction::St1wZPBzSX32Scaled, SVE_STORES_ACROSS_FAULT),
    (Instruction::St1bScatter64, SVE_STORES_ACROSS_FAULT),
    (Instruction::St1hScatter64, SVE_STORES_ACROSS_FAULT),
    (Instruction::St1wScatter64, SVE_STORES_ACROSS_FAULT),
    (Instruction::St1dScatter, SVE_STORES_ACROSS_FAULT),
    (Instruction::St1hScatterSCALED64, SVE_STORES_ACROSS_FAULT),
    (Instruction::St1wScatterSCALED64, SVE_STORES_ACROSS_FAULT),
    (Instruction::St1dScatterSCALED, SVE_STORES_ACROSS_FAULT),
    (Instruction::St1bScatterImmediate64, SVE_STORES_ACROSS_FAULT),
    (Instruction::St1hScatterImmediate64, SVE_STORES_ACROSS_FAULT),
    (Instruction::St1wScatterImmediate64, SVE_STORES_ACROSS_FAULT),
    (Instruction::St1dScatterImmediate, SVE_STORES_ACROSS_FAULT),
    (Instruction::St1bScatterImmediate, SVE_STORES_ACROSS_FAULT),
    (Instruction::St1hScatterImmediate, SVE_STORES_ACROSS_FAULT),
    (Instruction::St1wScatterImmediate, SVE_STORES_ACROSS_FAULT),
    (Instruction::St1bZPBi, SVE_STORES_ACROSS_FAULT),
    (Instruction::St1hZPBi, SVE_STORES_ACROSS_FAULT),
    (Instruction::St1wZPBi, SVE_STORES_ACROSS_FAULT),
    (Instruction::St1dImmediate, SVE_STORES_ACROSS_FAULT),
    (Instruction::Stnt1bZRI, SVE_STORES_ACROSS_FAULT),
    (Instruction::Stnt1hZRI, SVE_STORES_ACROSS_FAULT),
    (Instruction::Stnt1wZRI, SVE_STORES_ACROSS_FAULT),
    (Instruction::Stnt1dZRI, SVE_STORES_ACROSS_FAULT),
    (Instruction::St2bImmediate, SVE_STORES_ACROSS_FAULT),
    (Instruction::St3bImmediate, SVE_STORES_ACROSS_FAULT),
    (Instruction::St4bImmediate, SVE_STORES_ACROSS_FAULT),
    (Instruction::St2hImmediate, SVE_STORES_ACROSS_FAULT),
    (Instruction::St3hImmediate, SVE_STORES_ACROSS_FAULT),
    (Instruction::St4hImmediate, SVE_STORES_ACROSS_FAULT),
    (Instruction::St2wImmediate, SVE_STORES_ACROSS_FAULT),
    (Instruction::St3wImmediate, SVE_STORES_ACROSS_FAULT),
    (Instruction::St4wImmediate, SVE_STORES_ACROSS_FAULT),
    (Instruction::St2dImmediate, SVE_STORES_ACROSS_FAULT),
    (Instruction::St3dImmediate, SVE_STORES_ACROSS_FAULT),
    (Instruction::St4dImmediate, SVE_STORES_ACROSS_FAULT),
    (Instruction::Hint, POINTER_STRIP),
    (Instruction::Xpaci, POINTER_STRIP),
    (Instruction::Xpacd, POINTER_STRIP),
    (Instruction::Sys, SYSTEM_INSTRUCTION),
    (Instruction::MsrRegister, SYSTEM_REGISTERS),
    (Instruction::Mrs, SYSTEM_REGISTERS),
    (Instruction::Casp32, ATOMIC_ALIGNMENT),
    (Instruction::Caspl32, ATOMIC_ALIGNMENT),
    (Instruction::Caspa32, ATOMIC_ALIGNMENT),
    (Instruction::Caspal32, ATOMIC_ALIGNMENT),
    (Instruction::Cash, ATOMIC_ALIGNMENT),
    (Instruction::Caslh, ATOMIC_ALIGNMENT),
    (Instruction::Casah, ATOMIC_ALIGNMENT),
    (Instruction::Casalh, ATOMIC_ALIGNMENT),
    (Instruction::Cas32, ATOMIC_ALIGNMENT),
    (Instruction::Casl32, ATOMIC_ALIGNMENT),
    (Instruction::Casa32, ATOMIC_ALIGNMENT),
    (Instruction::Casal32, ATOMIC_ALIGNMENT),
    (Instruction::Cas64, ATOMIC_ALIGNMENT),
    (Instruction::Casl64, ATOMIC_ALIGNMENT),
    (Instruction::Casa64, ATOMIC_ALIGNMENT),
    (Instruction::Casal64, ATOMIC_ALIGNMENT),
    (Instruction::Ldaddh, ATOMIC_ALIGNMENT),
    (Instruction::Ldclrh, ATOMIC_ALIGNMENT),
    (Instruction::Ldeorh, ATOMIC_ALIGNMENT),
    (Instruction::Ldseth, ATOMIC_ALIGNMENT),
    (Instruction::Ldsmaxh, ATOMIC_ALIGNMENT),
    (Instruction::Ldsminh, ATOMIC_ALIGNMENT),
    (Instruction::Ldumaxh, ATOMIC_ALIGNMENT),
    (Instruction::Lduminh, ATOMIC_ALIGNMENT),
    (Instruction::Swph, ATOMIC_ALIGNMENT),
    (Instruction::Ldaddlh, ATOMIC_ALIGNMENT),
    (Instruction::Ldclrlh, ATOMIC_ALIGNMENT),
    (Instruction::Ldeorlh, ATOMIC_ALIGNMENT),
    (Instruction::Ldsetlh, ATOMIC_ALIGNMENT),
    (Instruction::Ldsmaxlh, ATOMIC_ALIGNMENT),
    (Instruction::Ldsminlh, ATOMIC_ALIGNMENT),
    (Instruction::Ldumaxlh, ATOMIC_ALIGNMENT),
    (Instruction::Lduminlh, ATOMIC_ALIGNMENT),
    (Instruction::Swplh, ATOMIC_ALIGNMENT),
    (Instruction::Ldaddah, ATOMIC_ALIGNMENT),
    (Instruction::Ldclrah, ATOMIC_ALIGNMENT),
    (Instruction::Ldeorah, ATOMIC_ALIGNMENT),
    (Instruction::Ldsetah, ATOMIC_ALIGNMENT),
    (Instruction::Ldsmaxah, ATOMIC_ALIGNMENT),
    (Instruction::Ldsminah, ATOMIC_ALIGNMENT),
    (Instruction::Ldumaxah, ATOMIC_ALIGNMENT),
    (Instruction::Lduminah, ATOMIC_ALIGNMENT),
    (Instruction::Swpah, ATOMIC_ALIGNMENT),
    (Instruction::Ldaddalh, ATOMIC_ALIGNMENT),
    (Instruction::Ldclralh, ATOMIC_ALIGNMENT),
    (Instruction::Ldeoralh, ATOMIC_ALIGNMENT),
    (Instruction::Ldsetalh, ATOMIC_ALIGNMENT),
    (Instruction::Ldsmaxalh, ATOMIC_ALIGNMENT),
    (Instruction::Ldsminalh, ATOMIC_ALIGNMENT),
    (Instruction::Ldumaxalh, ATOMIC_ALIGNMENT),
    (Instruction::Lduminalh, ATOMIC_ALIGNMENT),
    (Instruction::Swpalh, ATOMIC_ALIGNMENT),
    (Instruction::Ldadd32, ATOMIC_ALIGNMENT),
    (Instruction::Ldclr32, ATOMIC_ALIGNMENT),
    (Instruction::Ldeor32, ATOMIC_ALIGNMENT),
    (Instruction::Ldset32, ATOMIC_ALIGNMENT),
    (Instruction::Ldsmax32, ATOMIC_ALIGNMENT),
    (Instruction::Ldsmin32, ATOMIC_ALIGNMENT),
    (Instruction::Ldumax32, ATOMIC_ALIGNMENT),
    (Instruction::Ldumin32, ATOMIC_ALIGNMENT),
    (Instruction::Swp32, ATOMIC_ALIGNMENT),
    (Instruction::Ldaddl32, ATOMIC_ALIGNMENT),
    (Instruction::Ldclrl32, ATOMIC_ALIGNMENT),
    (Instruction::Ldeorl32, ATOMIC_ALIGNMENT),
    (Instruction::Ldsetl32, ATOMIC_ALIGNMENT),
    (Instruction::Ldsmaxl32, ATOMIC_ALIGNMENT),
    (Instruction::Ldsminl32, ATOMIC_ALIGNMENT),
    (Instruction::Ldumaxl32, ATOMIC_ALIGNMENT),
    (Instruction::Lduminl32, ATOMIC_ALIGNMENT),
    (Instruction::Swpl32, ATOMIC_ALIGNMENT),
    (Instruction::Ldadda32, ATOMIC_ALIGNMENT),
    (Instruction::Ldclra32, ATOMIC_ALIGNMENT),
    (Instruction::Ldeora32, ATOMIC_ALIGNMENT),
    (Instruction::Ldseta32, ATOMIC_ALIGNMENT),
    (Instruction::Ldsmaxa32, ATOMIC_ALIGNMENT),
    (Instruction::Ldsmina32, ATOMIC_ALIGNMENT),
    (Instruction::Ldumaxa32, ATOMIC_ALIGNMENT),
    (Instruction::Ldumina32, ATOMIC_ALIGNMENT),
    (Instruction::Swpa32, ATOMIC_ALIGNMENT),
    (Instruction::Ldaddal32, ATOMIC_ALIGNMENT),
    (Instruction::Ldclral32, ATOMIC_ALIGNMENT),
    (Instruction::Ldeoral32, ATOMIC_ALIGNMENT),
    (Instruction::Ldsetal32, ATOMIC_ALIGNMENT),
    (Instruction::Ldsmaxal32, ATOMIC_ALIGNMENT),
    (Instruction::Ldsminal32, ATOMIC_ALIGNMENT),
    (Instruction::Ldumaxal32, ATOMIC_ALIGNMENT),
    (Instruction::Lduminal32, ATOMIC_ALIGNMENT),
    (Instruction::Swpal32, ATOMIC_ALIGNMENT),
    (Instruction::Ldadd64, ATOMIC_ALIGNMENT),
    (Instruction::Ldclr64, ATOMIC_ALIGNMENT),
    (Instruction::Ldeor64, ATOMIC_ALIGNMENT),
    (Instruction::Ldset64, ATOMIC_ALIGNMENT),
    (Instruction::Ldsmax64, ATOMIC_ALIGNMENT),
    (Instruction::Ldsmin64, ATOMIC_ALIGNMENT),
    (Instruction::Ldumax64, ATOMIC_ALIGNMENT),
    (Instruction::Ldumin64, ATOMIC_ALIGNMENT),
    (Instruction::Swp64, ATOMIC_ALIGNMENT),
    (Instruction::Ldaddl64, ATOMIC_ALIGNMENT),
    (Instruction::Ldclrl64, ATOMIC_ALIGNMENT),
    (Instruction::Ldeorl64, ATOMIC_ALIGNMENT),
    (Instruction::Ldsetl64, ATOMIC_ALIGNMENT),
    (Instruction::Ldsmaxl64, ATOMIC_ALIGNMENT),
    (Instruction::Ldsminl64, ATOMIC_ALIGNMENT),
    (Instruction::Ldumaxl64, ATOMIC_ALIGNMENT),
    (Instruction::Lduminl64, ATOMIC_ALIGNMENT),
    (Instruction::Swpl64, ATOMIC_ALIGNMENT),
    (Instruction::Ldadda64, ATOMIC_ALIGNMENT),
    (Instruction::Ldclra64, ATOMIC_ALIGNMENT),
    (Instruction::Ldeora64, ATOMIC_ALIGNMENT),
    (Instruction::Ldseta64, ATOMIC_ALIGNMENT),
    (Instruction::Ldsmaxa64, ATOMIC_ALIGNMENT),
    (Instruction::Ldsmina64, ATOMIC_ALIGNMENT),
    (Instruction::Ldumaxa64, ATOMIC_ALIGNMENT),
    (Instruction::Ldumina64, ATOMIC_ALIGNMENT),
    (Instruction::Swpa64, ATOMIC_ALIGNMENT),
    (Instruction::Ldaddal64, ATOMIC_ALIGNMENT),
    (Instruction::Ldclral64, ATOMIC_ALIGNMENT),
    (Instruction::Ldeoral64, ATOMIC_ALIGNMENT),
    (Instruction::Ldsetal64, ATOMIC_ALIGNMENT),
    (Instruction::Ldsmaxal64, ATOMIC_ALIGNMENT),
    (Instruction::Ldsminal64, ATOMIC_ALIGNMENT),
    (Instruction::Ldumaxal64, ATOMIC_ALIGNMENT),
    (Instruction::Lduminal64, ATOMIC_ALIGNMENT),
    (Instruction::Swpal64, ATOMIC_ALIGNMENT),
    (Instruction::Stxp32, EXCLUSIVE_STORE_ALIGNMENT),
    (Instruction::Stlxp32, EXCLUSIVE_STORE_ALIGNMENT),
    (Instruction::Stxp64, EXCLUSIVE_STORE_ALIGNMENT),
    (Instruction::Stlxp64, EXCLUSIVE_STORE_ALIGNMENT),
    (Instruction::Stxrh, EXCLUSIVE_STORE_ALIGNMENT),
    (Instruction::Stlxrh, EXCLUSIVE_STORE_ALIGNMENT),
    (Instruction::Stxr32, EXCLUSIVE_STORE_ALIGNMENT),
    (Instruction::Stlxr32, EXCLUSIVE_STORE_ALIGNMENT),
    (Instruction::Stxr64, EXCLUSIVE_STORE_ALIGNMENT),
    (Instruction::Stlxr64, EXCLUSIVE_STORE_ALIGNMENT),
    (Instruction::Ldxp32, EXCLUSIVE_LOAD_BEHAVIORS),
    (Instruction::Ldaxp32, EXCLUSIVE_LOAD_BEHAVIORS),
    (Instruction::Ldxrh, EXCLUSIVE_LOAD_BEHAVIORS),
    (Instruction::Ldaxrh, EXCLUSIVE_LOAD_BEHAVIORS),
    (Instruction::Ldxr32, EXCLUSIVE_LOAD_BEHAVIORS),
    (Instruction::Ldaxr32, EXCLUSIVE_LOAD_BEHAVIORS),
    (Instruction::Ldxr64, EXCLUSIVE_LOAD_BEHAVIORS),
    (Instruction::Ldaxr64, EXCLUSIVE_LOAD_BEHAVIORS),
    (Instruction::Stllrh, ORDERED_ALIGNMENT),
    (Instruction::Stlrh, ORDERED_ALIGNMENT),
    (Instruction::Ldlarh, ORDERED_ALIGNMENT),
    (Instruction::Ldarh, ORDERED_ALIGNMENT),
    (Instruction::Stllr32, ORDERED_ALIGNMENT),
    (Instruction::Stlr32, ORDERED_ALIGNMENT),
    (Instruction::Ldlar32, ORDERED_ALIGNMENT),
    (Instruction::Ldar32, ORDERED_ALIGNMENT),
    (Instruction::Stllr64, ORDERED_ALIGNMENT),
    (Instruction::Stlr64, ORDERED_ALIGNMENT),
    (Instruction::Ldlar64, ORDERED_ALIGNMENT),
    (Instruction::Ldar64, ORDERED_ALIGNMENT),
    (Instruction::Stlurh, ORDERED_ALIGNMENT),
    (Instruction::Ldapurh, ORDERED_ALIGNMENT),
    (Instruction::Ldapursh64, ORDERED_ALIGNMENT),
    (Instruction::Ldapursh32, ORDERED_ALIGNMENT),
    (Instruction::Stlur32, ORDERED_ALIGNMENT),
    (Instruction::Ldapur32, ORDERED_ALIGNMENT),
    (Instruction::Ldapursw, ORDERED_ALIGNMENT),
    (Instruction::Stlur64, ORDERED_ALIGNMENT),
    (Instruction::Ldapur64, ORDERED_ALIGNMENT),
    (Instruction::Ldaprh, ORDERED_ALIGNMENT),
    (Instruction::Ldapr32, ORDERED_ALIGNMENT),
    (Instruction::Ldapr64, ORDERED_ALIGNMENT),
    (Instruction::Ldsmaxb, SIGNED_BYTE_ATOMICS),
    (Instruction::Ldsminb, SIGNED_BYTE_ATOMICS),
    (Instruction::Ldsmaxlb, SIGNED_BYTE_ATOMICS),
    (Instruction::Ldsminlb, SIGNED_BYTE_ATOMICS),
    (Instruction::Ldsmaxab, SIGNED_BYTE_ATOMICS),
    (Instruction::Ldsminab, SIGNED_BYTE_ATOMICS),
    (Instruction::Ldsmaxalb, SIGNED_BYTE_ATOMICS),
    (Instruction::Ldsminalb, SIGNED_BYTE_ATOMICS),
    (Instruction::FcvtzsScalarImmediate, FLOATING_POINT),
    (Instruction::FcmlaAdvsimdElt, FLOATING_POINT),
    (Instruction::Fjcvtzs, FLOATING_POINT),
    (Instruction::Sm3tt1a, ABSENT_FEATURES),
    (Instruction::Sm3tt1b, ABSENT_FEATURES),
    (Instruction::Sm3tt2a, ABSENT_FEATURES),
    (Instruction::Sm3tt2b, ABSENT_FEATURES),
    (Instruction::Sm3partw1, ABSENT_FEATURES),
    (Instruction::Sm3partw2, ABSENT_FEATURES),
    (Instruction::Sm4ekeyVectorSM4ENCKEY, ABSENT_FEATURES),
    (Instruction::Sm3ss1, ABSENT_FEATURES),
    (Instruction::Sm4eVector, ABSENT_FEATURES),
];

wcontainers_executor_conformance::tests!(
    StandIn::new(),
    list_stand_in_behaviors(),
    CriticalValueCut::default(),
    list_unexecutable_states(),
    &SET_ASIDE
);
