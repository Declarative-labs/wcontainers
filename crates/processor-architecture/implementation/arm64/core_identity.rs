/// The two registers of an ARM64 core that name its design and its revision.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct CoreIdentity {
    /// [MIDR_EL1](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/MIDR-EL1--Main-ID-Register).
    pub main_id_register: u64,
    /// [REVIDR_EL1](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/REVIDR-EL1--Revision-ID-Register).
    pub revision_id_register: u64,
}
