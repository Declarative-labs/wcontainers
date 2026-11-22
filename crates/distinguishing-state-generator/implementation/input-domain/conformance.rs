use wcontainers_distinguishing_state_generator_input_domain::CriticalValueProduct;
use wcontainers_instruction_input_domain_arm64::Arm64InputDomain;
use wcontainers_processor_architecture::ProcessorArchitecture;
use wcontainers_processor_architecture_arm64::{
    Arm64, Instruction, LoadIntoBaseWithWriteback, PreciseInstruction, StorePairAcrossFault,
    VectorLength, VectorLengthReading,
};

/// Every behavior the ARM64 crate defines, with two cache types that differ in their lowest bit.
const BEHAVIORS: [PreciseInstruction; 21] = [
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
    PreciseInstruction::Rdvl(VectorLengthReading::Reads(VectorLength::Bits256)),
    PreciseInstruction::Rdvl(VectorLengthReading::Reads(VectorLength::Bits512)),
    PreciseInstruction::Rdvl(VectorLengthReading::Reads(VectorLength::Bits1024)),
    PreciseInstruction::Rdvl(VectorLengthReading::Reads(VectorLength::Bits2048)),
    PreciseInstruction::MrsCtrEl0 { cache_type: 0 },
    PreciseInstruction::MrsCtrEl0 { cache_type: 1 },
    PreciseInstruction::MrsCtrEl0 {
        cache_type: u64::MAX,
    },
];

fn candidates() -> Vec<(Instruction, Vec<PreciseInstruction>)> {
    Arm64::list_instructions()
        .map(|instruction| {
            let of_instruction = BEHAVIORS
                .into_iter()
                .filter(|precise| precise.to_instruction() == instruction);
            (instruction, of_instruction.collect())
        })
        .collect()
}

wcontainers_distinguishing_state_generator_conformance::tests!(
    Arm64,
    CriticalValueProduct::new(Arm64InputDomain),
    candidates()
);
