use vstd::prelude::*;
use wcontainers_processor_architecture::ProcessorArchitecture;

/// The collector of what each kind of core of a machine shows for every instruction.
verus! {

pub trait ObservationCollector {
    type Architecture: ProcessorArchitecture;
    /// What a core says it is, read from the core itself.
    type CoreIdentity: Clone + Eq;
    type Error;

    /// Returns one sample for each kind of core of the machine.
    fn collect_samples(
        &self,
    ) -> Result<Vec<CoreSample<Self::Architecture, Self::CoreIdentity>>, Self::Error>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
}

/// What one kind of core shows for every instruction of its architecture.
pub struct CoreSample<Architecture: ProcessorArchitecture, CoreIdentity> {
    pub core: CoreIdentity,
    /// Each instruction of the architecture with the one behavior the core shows for it.
    pub measurements: Vec<(Architecture::Instruction, Architecture::PreciseInstruction)>,
}

} // verus!
