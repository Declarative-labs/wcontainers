use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_distinguishing_state_generator_input_domain::CriticalValueProduct;
use wcontainers_processor_architecture::ExecutionState;
use wcontainers_processor_architecture_arm64::{
    Arm64, LoadIntoBaseWithWriteback, PreciseInstruction, StorePairAcrossFault, VectorLength,
    VectorLengthReading,
};

/// One precise instruction for each behavior of each instruction.
const BEHAVIORS: [PreciseInstruction; 15] = [
    PreciseInstruction::AddShiftedRegister64,
    PreciseInstruction::AddsShiftedRegister64,
    PreciseInstruction::Csel64,
    PreciseInstruction::LdrImmediateUnsignedOffset64,
    PreciseInstruction::StrImmediateUnsignedOffset64,
    PreciseInstruction::StpSignedOffset64(StorePairAcrossFault::WritesNothing),
    PreciseInstruction::StpSignedOffset64(StorePairAcrossFault::WritesBytesBelowFault),
    PreciseInstruction::LdrImmediatePostIndex64(LoadIntoBaseWithWriteback::WritebackSuppressed),
    PreciseInstruction::LdrImmediatePostIndex64(LoadIntoBaseWithWriteback::WritebackOverwritesLoad),
    PreciseInstruction::LdrImmediatePostIndex64(LoadIntoBaseWithWriteback::Undefined),
    PreciseInstruction::LdrImmediatePostIndex64(LoadIntoBaseWithWriteback::NoOperation),
    PreciseInstruction::Brk,
    PreciseInstruction::Rdvl(VectorLengthReading::Undefined),
    PreciseInstruction::Rdvl(VectorLengthReading::Reads(VectorLength::Bits128)),
    PreciseInstruction::MrsCtrEl0 {
        cache_type: u64::MAX,
    },
];

/// Each behavior on every state the critical value generator makes for its instruction.
fn cases() -> impl Iterator<Item = (PreciseInstruction, ExecutionState<'static, Arm64>)> {
    BEHAVIORS.into_iter().flat_map(|precise| {
        let states = DistinguishingStateGenerator::<Arm64>::generate_states(
            &CriticalValueProduct,
            precise.to_instruction(),
        );
        states.map(move |state| (precise, state))
    })
}

wcontainers_processor_architecture_conformance::tests!(cases());
