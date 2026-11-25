//! [Hypervisor.framework](https://developer.apple.com/documentation/hypervisor), Apple's virtual machines, on Apple silicon.
#![cfg(target_arch = "aarch64")]

use applevisor_sys::{
    hv_cache_type_t, hv_feature_reg_t, hv_gic_distributor_reg_t, hv_gic_icc_reg_t,
    hv_gic_ich_reg_t, hv_gic_icv_reg_t, hv_gic_intid_t, hv_gic_msi_reg_t,
    hv_gic_redistributor_reg_t, hv_interrupt_type_t, hv_ipa_granule_t, hv_memory_flags_t, hv_reg_t,
    hv_return_t, hv_simd_fp_reg_t, hv_sme_p_reg_t, hv_sme_z_reg_t, hv_sme_zt0_uchar64_t,
    hv_sys_reg_t, hv_vcpu_exit_t, hv_vcpu_sme_state_t, hv_vcpu_t,
};
use wcontainers_volatile_bytes::VolatileBytes;

/// Every function of [Hypervisor.framework](https://developer.apple.com/documentation/hypervisor) on ARM64, each one method named without its `hv_` prefix.
///
/// The process has one VM at a time; a call the framework documents as the VM's takes it, so that it cannot outlive it.
#[allow(
    reviewable::explicit_shared_error::Failure,
    reason = "every function of the framework fails the same way, with its return code"
)]
pub trait HypervisorFramework {
    type VmConfig;
    type VcpuConfig;
    type GicConfig;
    type GicState;
    type Vm;
    /// A vCPU, which only the thread that made it may use or end.
    type Vcpu;
    type Memory: VolatileBytes;

    fn vm_config_create(&self) -> Result<Self::VmConfig, Failure>;
    fn vm_config_get_max_ipa_size(&self) -> Result<u32, Failure>;
    fn vm_config_get_default_ipa_size(&self) -> Result<u32, Failure>;
    fn vm_config_set_ipa_size(
        &self,
        config: &mut Self::VmConfig,
        ipa_bit_length: u32,
    ) -> Result<(), Failure>;
    fn vm_config_get_ipa_size(&self, config: &Self::VmConfig) -> Result<u32, Failure>;
    fn vm_config_get_el2_supported(&self) -> Result<bool, Failure>;
    fn vm_config_get_el2_enabled(&self, config: &Self::VmConfig) -> Result<bool, Failure>;
    fn vm_config_set_el2_enabled(
        &self,
        config: &mut Self::VmConfig,
        el2_enabled: bool,
    ) -> Result<(), Failure>;
    fn vm_config_get_default_ipa_granule(&self) -> Result<IpaGranule, Failure>;
    fn vm_config_get_ipa_granule(&self, config: &Self::VmConfig) -> Result<IpaGranule, Failure>;
    fn vm_config_set_ipa_granule(
        &self,
        config: &mut Self::VmConfig,
        granule: IpaGranule,
    ) -> Result<(), Failure>;

    /// Makes [the process's VM](https://developer.apple.com/documentation/hypervisor/hv_vm_create(_:)), which ends when it, its vCPUs and its memory views are dropped.
    fn vm_create(&self, config: Option<&Self::VmConfig>) -> Result<Self::Vm, Failure>;
    fn vm_get_max_vcpu_count(&self) -> Result<u32, Failure>;

    /// Maps zeroed host memory of the length, in whole host pages, that `vm_map` can give the VM.
    fn create_user_memory(&self, length: usize) -> Result<Self::Memory, Failure>;

    /// Maps [the memory into the VM](https://developer.apple.com/documentation/hypervisor/hv_vm_map(_:_:_:_:)), which keeps it mapped until a `vm_unmap` covers all of it.
    fn vm_map(
        &self,
        vm: &Self::Vm,
        memory: &Self::Memory,
        ipa: u64,
        flags: MemoryFlags,
    ) -> Result<(), Failure>;
    fn vm_unmap(&self, vm: &Self::Vm, ipa: u64, size: usize) -> Result<(), Failure>;
    fn vm_protect(
        &self,
        vm: &Self::Vm,
        ipa: u64,
        size: usize,
        flags: MemoryFlags,
    ) -> Result<(), Failure>;

    fn vcpu_config_create(&self) -> Result<Self::VcpuConfig, Failure>;
    fn vcpu_config_get_feature_reg(
        &self,
        config: &Self::VcpuConfig,
        feature_reg: FeatureReg,
    ) -> Result<u64, Failure>;
    fn vcpu_config_get_ccsidr_el1_sys_reg_values(
        &self,
        config: &Self::VcpuConfig,
        cache_type: CacheType,
    ) -> Result<CacheSizes, Failure>;

    fn vcpu_create(
        &self,
        vm: &Self::Vm,
        config: Option<&Self::VcpuConfig>,
    ) -> Result<Self::Vcpu, Failure>;
    fn vcpu_run(&self, vcpu: &Self::Vcpu) -> Result<(), Failure>;

    /// Returns [why the vCPU last stopped running](https://developer.apple.com/documentation/hypervisor/hv_vcpu_exit_t).
    fn vcpu_exit(&self, vcpu: &Self::Vcpu) -> VcpuExit;

    /// Returns the vCPU's identifier, which `vcpus_exit` takes on any thread.
    fn vcpu_id(&self, vcpu: &Self::Vcpu) -> VcpuId;
    fn vcpus_exit(&self, vcpus: &[VcpuId]) -> Result<(), Failure>;
    fn vcpu_get_pending_interrupt(
        &self,
        vcpu: &Self::Vcpu,
        interrupt: InterruptType,
    ) -> Result<bool, Failure>;
    fn vcpu_set_pending_interrupt(
        &self,
        vcpu: &Self::Vcpu,
        interrupt: InterruptType,
        pending: bool,
    ) -> Result<(), Failure>;
    fn vcpu_get_exec_time(&self, vcpu: &Self::Vcpu) -> Result<u64, Failure>;
    fn vcpu_get_reg(&self, vcpu: &Self::Vcpu, reg: Reg) -> Result<u64, Failure>;
    fn vcpu_set_reg(&self, vcpu: &Self::Vcpu, reg: Reg, value: u64) -> Result<(), Failure>;
    fn vcpu_get_simd_fp_reg(&self, vcpu: &Self::Vcpu, reg: SimdFpReg) -> Result<u128, Failure>;
    fn vcpu_set_simd_fp_reg(
        &self,
        vcpu: &Self::Vcpu,
        reg: SimdFpReg,
        value: u128,
    ) -> Result<(), Failure>;
    fn vcpu_get_sys_reg(&self, vcpu: &Self::Vcpu, reg: SysReg) -> Result<u64, Failure>;
    fn vcpu_set_sys_reg(&self, vcpu: &Self::Vcpu, reg: SysReg, value: u64) -> Result<(), Failure>;
    fn vcpu_get_trap_debug_exceptions(&self, vcpu: &Self::Vcpu) -> Result<bool, Failure>;
    fn vcpu_set_trap_debug_exceptions(&self, vcpu: &Self::Vcpu, value: bool)
    -> Result<(), Failure>;
    fn vcpu_get_trap_debug_reg_accesses(&self, vcpu: &Self::Vcpu) -> Result<bool, Failure>;
    fn vcpu_set_trap_debug_reg_accesses(
        &self,
        vcpu: &Self::Vcpu,
        value: bool,
    ) -> Result<(), Failure>;
    fn vcpu_get_vtimer_mask(&self, vcpu: &Self::Vcpu) -> Result<bool, Failure>;
    fn vcpu_set_vtimer_mask(
        &self,
        vcpu: &Self::Vcpu,
        vtimer_is_masked: bool,
    ) -> Result<(), Failure>;
    fn vcpu_get_vtimer_offset(&self, vcpu: &Self::Vcpu) -> Result<u64, Failure>;
    fn vcpu_set_vtimer_offset(&self, vcpu: &Self::Vcpu, vtimer_offset: u64) -> Result<(), Failure>;

    fn sme_config_get_max_svl_bytes(&self) -> Result<usize, Failure>;
    fn vcpu_get_sme_state(&self, vcpu: &Self::Vcpu) -> Result<SmeState, Failure>;
    fn vcpu_set_sme_state(&self, vcpu: &Self::Vcpu, sme_state: &SmeState) -> Result<(), Failure>;
    fn vcpu_get_sme_z_reg(
        &self,
        vcpu: &Self::Vcpu,
        reg: SmeZReg,
        value: &mut [u8],
    ) -> Result<(), Failure>;
    fn vcpu_set_sme_z_reg(
        &self,
        vcpu: &Self::Vcpu,
        reg: SmeZReg,
        value: &[u8],
    ) -> Result<(), Failure>;
    fn vcpu_get_sme_p_reg(
        &self,
        vcpu: &Self::Vcpu,
        reg: SmePReg,
        value: &mut [u8],
    ) -> Result<(), Failure>;
    fn vcpu_set_sme_p_reg(
        &self,
        vcpu: &Self::Vcpu,
        reg: SmePReg,
        value: &[u8],
    ) -> Result<(), Failure>;
    fn vcpu_get_sme_za_reg(&self, vcpu: &Self::Vcpu, value: &mut [u8]) -> Result<(), Failure>;
    fn vcpu_set_sme_za_reg(&self, vcpu: &Self::Vcpu, value: &[u8]) -> Result<(), Failure>;
    fn vcpu_get_sme_zt0_reg(&self, vcpu: &Self::Vcpu) -> Result<SmeZt0, Failure>;
    fn vcpu_set_sme_zt0_reg(&self, vcpu: &Self::Vcpu, value: &SmeZt0) -> Result<(), Failure>;

    fn gic_config_create(&self) -> Result<Self::GicConfig, Failure>;
    fn gic_config_set_distributor_base(
        &self,
        config: &mut Self::GicConfig,
        base: u64,
    ) -> Result<(), Failure>;
    fn gic_config_set_redistributor_base(
        &self,
        config: &mut Self::GicConfig,
        base: u64,
    ) -> Result<(), Failure>;
    fn gic_config_set_msi_region_base(
        &self,
        config: &mut Self::GicConfig,
        base: u64,
    ) -> Result<(), Failure>;
    fn gic_config_set_msi_interrupt_range(
        &self,
        config: &mut Self::GicConfig,
        msi_intid_base: u32,
        msi_intid_count: u32,
    ) -> Result<(), Failure>;
    fn gic_create(&self, vm: &Self::Vm, config: &Self::GicConfig) -> Result<(), Failure>;
    fn gic_set_spi(&self, vm: &Self::Vm, intid: u32, level: bool) -> Result<(), Failure>;
    fn gic_send_msi(&self, vm: &Self::Vm, address: u64, intid: u32) -> Result<(), Failure>;
    fn gic_get_distributor_reg(
        &self,
        vm: &Self::Vm,
        reg: GicDistributorReg,
    ) -> Result<u64, Failure>;
    fn gic_set_distributor_reg(
        &self,
        vm: &Self::Vm,
        reg: GicDistributorReg,
        value: u64,
    ) -> Result<(), Failure>;
    fn gic_get_redistributor_base(&self, vcpu: &Self::Vcpu) -> Result<u64, Failure>;
    fn gic_get_redistributor_reg(
        &self,
        vcpu: &Self::Vcpu,
        reg: GicRedistributorReg,
    ) -> Result<u64, Failure>;
    fn gic_set_redistributor_reg(
        &self,
        vcpu: &Self::Vcpu,
        reg: GicRedistributorReg,
        value: u64,
    ) -> Result<(), Failure>;
    fn gic_get_icc_reg(&self, vcpu: &Self::Vcpu, reg: GicIccReg) -> Result<u64, Failure>;
    fn gic_set_icc_reg(&self, vcpu: &Self::Vcpu, reg: GicIccReg, value: u64)
    -> Result<(), Failure>;
    fn gic_get_ich_reg(&self, vcpu: &Self::Vcpu, reg: GicIchReg) -> Result<u64, Failure>;
    fn gic_set_ich_reg(&self, vcpu: &Self::Vcpu, reg: GicIchReg, value: u64)
    -> Result<(), Failure>;
    fn gic_get_icv_reg(&self, vcpu: &Self::Vcpu, reg: GicIcvReg) -> Result<u64, Failure>;
    fn gic_set_icv_reg(&self, vcpu: &Self::Vcpu, reg: GicIcvReg, value: u64)
    -> Result<(), Failure>;
    fn gic_get_msi_reg(&self, vm: &Self::Vm, reg: GicMsiReg) -> Result<u64, Failure>;
    fn gic_set_msi_reg(&self, vm: &Self::Vm, reg: GicMsiReg, value: u64) -> Result<(), Failure>;
    fn gic_set_state(&self, vm: &Self::Vm, gic_state_data: &[u8]) -> Result<(), Failure>;
    fn gic_reset(&self, vm: &Self::Vm) -> Result<(), Failure>;
    fn gic_get_distributor_size(&self) -> Result<usize, Failure>;
    fn gic_get_distributor_base_alignment(&self) -> Result<usize, Failure>;
    fn gic_get_redistributor_region_size(&self) -> Result<usize, Failure>;
    fn gic_get_redistributor_size(&self) -> Result<usize, Failure>;
    fn gic_get_redistributor_base_alignment(&self) -> Result<usize, Failure>;
    fn gic_get_msi_region_size(&self) -> Result<usize, Failure>;
    fn gic_get_msi_region_base_alignment(&self) -> Result<usize, Failure>;

    /// Returns the first shared interrupt's number and how many there are.
    fn gic_get_spi_interrupt_range(&self) -> Result<(u32, u32), Failure>;
    fn gic_get_intid(&self, interrupt: GicIntId) -> Result<u32, Failure>;
    fn gic_state_create(&self, vm: &Self::Vm) -> Result<Self::GicState, Failure>;
    fn gic_state_get_size(&self, state: &Self::GicState) -> Result<usize, Failure>;
    fn gic_state_get_data(&self, state: &Self::GicState) -> Result<Vec<u8>, Failure>;
}

/// A return code of the framework other than `HV_SUCCESS`.
pub type Failure = hv_return_t;

pub type IpaGranule = hv_ipa_granule_t;

pub type MemoryFlags = hv_memory_flags_t;

pub type FeatureReg = hv_feature_reg_t;

pub type CacheType = hv_cache_type_t;

/// The values of CCSIDR_EL1 a vCPU configuration gives, one for each cache level the framework names.
pub type CacheSizes = [u64; CACHE_LEVELS];

/// The cache levels the framework gives CCSIDR_EL1 values for.
///
/// https://developer.apple.com/documentation/hypervisor/hv_vcpu_config_get_ccsidr_el1_sys_reg_values(_:_:_:)
pub const CACHE_LEVELS: usize = 8;

pub type VcpuExit = hv_vcpu_exit_t;

pub type VcpuId = hv_vcpu_t;

pub type InterruptType = hv_interrupt_type_t;

pub type Reg = hv_reg_t;

pub type SimdFpReg = hv_simd_fp_reg_t;

pub type SysReg = hv_sys_reg_t;

pub type SmeState = hv_vcpu_sme_state_t;

pub type SmeZReg = hv_sme_z_reg_t;

pub type SmePReg = hv_sme_p_reg_t;

pub type SmeZt0 = hv_sme_zt0_uchar64_t;

pub type GicDistributorReg = hv_gic_distributor_reg_t;

pub type GicRedistributorReg = hv_gic_redistributor_reg_t;

pub type GicIccReg = hv_gic_icc_reg_t;

pub type GicIchReg = hv_gic_ich_reg_t;

pub type GicIcvReg = hv_gic_icv_reg_t;

pub type GicMsiReg = hv_gic_msi_reg_t;

pub type GicIntId = hv_gic_intid_t;
