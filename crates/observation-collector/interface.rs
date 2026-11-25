use wcontainers_processor_architecture::ProcessorArchitecture;

/// The collector of what each kind of core of a machine shows for every instruction.
pub trait ObservationCollector {
    type Architecture: ProcessorArchitecture;
    /// What a core says it is, read from the core itself.
    type CoreIdentity: Clone + Eq;
    type Error;

    /// Returns one sample for each kind of core of the machine.
    fn collect_samples(
        &self,
    ) -> Result<Vec<Sample<Self::Architecture, Self::CoreIdentity>>, Self::Error>;
}

/// What one kind of core shows for every instruction of its architecture.
pub struct Sample<Architecture: ProcessorArchitecture, CoreIdentity> {
    pub core: CoreIdentity,
    /// Each instruction of the architecture with the one behavior the core shows for it.
    pub measurements: Vec<(Architecture::Instruction, Architecture::PreciseInstruction)>,
}
