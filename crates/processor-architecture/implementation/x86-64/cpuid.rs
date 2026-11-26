//! What CPUID answers, which belongs to a chip (or the hypervisor it runs under), never to a state.

/// One answer of CPUID.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_x86_64)
)]
pub struct CpuidLeaf {
    pub leaf: u32,
    /// The ECX the answer is for, or `None` when the leaf ignores ECX.
    pub subleaf: Option<u32>,
    /// EAX, EBX, ECX and EDX.
    pub registers: [u32; 4],
}

/// The answers a chip gives, for the leaves it has; a leaf the table does not hold answers by the rule of
/// [`CpuidTable::read`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_x86_64)
)]
pub struct CpuidTable {
    pub leaves: &'static [CpuidLeaf],
}

/// The first leaf of the hypervisor range, which only a hypervisor answers.
const HYPERVISOR_RANGE: u32 = 0x4000_0000;
const HYPERVISOR_RANGE_END: u32 = 0x5000_0000;
const EXTENDED_RANGE: u32 = 0x8000_0000;

impl CpuidTable {
    /// The answer for a leaf and subleaf.
    ///
    /// A leaf above the highest of its range answers as the [highest basic leaf](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html) (Intel's rule; CPUID's
    /// description in volume 2A); a leaf within its range that the table does not hold answers zeros.
    pub fn read(&self, leaf: u32, subleaf: u32) -> [u32; 4] {
        if let Some(registers) = self.find(leaf, subleaf) {
            return registers;
        }
        let range = if leaf >= EXTENDED_RANGE {
            EXTENDED_RANGE
        } else if (HYPERVISOR_RANGE..HYPERVISOR_RANGE_END).contains(&leaf) {
            HYPERVISOR_RANGE
        } else {
            0
        };
        let within_range = self
            .find(range, 0)
            .is_some_and(|[highest, ..]| leaf <= highest);
        if within_range {
            return [0; 4];
        }
        let highest_basic = self.find(0, 0).map_or(0, |[highest, ..]| highest);
        self.find(highest_basic, subleaf).unwrap_or([0; 4])
    }

    fn find(&self, leaf: u32, subleaf: u32) -> Option<[u32; 4]> {
        self.leaves
            .iter()
            .find(|answer| answer.leaf == leaf && answer.subleaf.is_none_or(|own| own == subleaf))
            .map(|answer| answer.registers)
    }
}

/// An Intel Xeon Platinum 8488C under KVM (Linux 7.0, an AWS bare metal machine), as the chip source's guest reads
/// it: recorded by the `record_cpuid` example, for the leaves the generated states ask and the highest basic leaf.
pub static XEON_PLATINUM_8488C_UNDER_KVM: CpuidTable = CpuidTable {
    leaves: &crate::cpuid_recording::LEAVES,
};
