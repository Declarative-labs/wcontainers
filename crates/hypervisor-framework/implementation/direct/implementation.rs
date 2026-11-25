//! Hypervisor.framework's functions on this Mac, each one marked call through applevisor-sys.
#![cfg(target_arch = "aarch64")]

use std::ffi::c_void;
use std::marker::PhantomData;
use std::mem::offset_of;
use std::ptr;
use std::sync::{Arc, Mutex, PoisonError};

use applevisor_sys::{
    hv_error_t, hv_exit_reason_t, hv_gic_config_create, hv_gic_config_set_distributor_base,
    hv_gic_config_set_msi_interrupt_range, hv_gic_config_set_msi_region_base,
    hv_gic_config_set_redistributor_base, hv_gic_config_t, hv_gic_create,
    hv_gic_get_distributor_base_alignment, hv_gic_get_distributor_reg, hv_gic_get_distributor_size,
    hv_gic_get_icc_reg, hv_gic_get_ich_reg, hv_gic_get_icv_reg, hv_gic_get_intid,
    hv_gic_get_msi_reg, hv_gic_get_msi_region_base_alignment, hv_gic_get_msi_region_size,
    hv_gic_get_redistributor_base, hv_gic_get_redistributor_base_alignment,
    hv_gic_get_redistributor_reg, hv_gic_get_redistributor_region_size,
    hv_gic_get_redistributor_size, hv_gic_get_spi_interrupt_range, hv_gic_reset, hv_gic_send_msi,
    hv_gic_set_distributor_reg, hv_gic_set_icc_reg, hv_gic_set_ich_reg, hv_gic_set_icv_reg,
    hv_gic_set_msi_reg, hv_gic_set_redistributor_reg, hv_gic_set_spi, hv_gic_set_state,
    hv_gic_state_create, hv_gic_state_get_data, hv_gic_state_get_size, hv_gic_state_t,
    hv_ipa_granule_t, hv_return_t, hv_sme_config_get_max_svl_bytes, hv_vcpu_config_create,
    hv_vcpu_config_get_ccsidr_el1_sys_reg_values, hv_vcpu_config_get_feature_reg, hv_vcpu_config_t,
    hv_vcpu_create, hv_vcpu_destroy, hv_vcpu_exit_exception_t, hv_vcpu_exit_t,
    hv_vcpu_get_exec_time, hv_vcpu_get_pending_interrupt, hv_vcpu_get_reg, hv_vcpu_get_simd_fp_reg,
    hv_vcpu_get_sme_p_reg, hv_vcpu_get_sme_state, hv_vcpu_get_sme_z_reg, hv_vcpu_get_sme_za_reg,
    hv_vcpu_get_sme_zt0_reg, hv_vcpu_get_sys_reg, hv_vcpu_get_trap_debug_exceptions,
    hv_vcpu_get_trap_debug_reg_accesses, hv_vcpu_get_vtimer_mask, hv_vcpu_get_vtimer_offset,
    hv_vcpu_run, hv_vcpu_set_pending_interrupt, hv_vcpu_set_reg, hv_vcpu_set_sme_p_reg,
    hv_vcpu_set_sme_state, hv_vcpu_set_sme_z_reg, hv_vcpu_set_sme_za_reg, hv_vcpu_set_sme_zt0_reg,
    hv_vcpu_set_sys_reg, hv_vcpu_set_trap_debug_exceptions, hv_vcpu_set_trap_debug_reg_accesses,
    hv_vcpu_set_vtimer_mask, hv_vcpu_set_vtimer_offset, hv_vcpu_t, hv_vcpus_exit,
    hv_vm_config_create, hv_vm_config_get_default_ipa_granule, hv_vm_config_get_default_ipa_size,
    hv_vm_config_get_el2_enabled, hv_vm_config_get_el2_supported, hv_vm_config_get_ipa_granule,
    hv_vm_config_get_ipa_size, hv_vm_config_get_max_ipa_size, hv_vm_config_set_el2_enabled,
    hv_vm_config_set_ipa_granule, hv_vm_config_set_ipa_size, hv_vm_config_t, hv_vm_create,
    hv_vm_destroy, hv_vm_get_max_vcpu_count, hv_vm_map, hv_vm_protect, hv_vm_unmap, os_release,
};
use wcontainers_hypervisor_framework::{
    CACHE_LEVELS, CacheSizes, CacheType, Failure, FeatureReg, GicDistributorReg, GicIccReg,
    GicIchReg, GicIcvReg, GicIntId, GicMsiReg, GicRedistributorReg, HypervisorFramework,
    InterruptType, IpaGranule, MemoryFlags, Reg, SimdFpReg, SmePReg, SmeState, SmeZReg, SmeZt0,
    SysReg, VcpuExit, VcpuId,
};
use wcontainers_volatile_bytes::{StandaloneVolatileBytes, VolatileBytes};
use wcontainers_volatile_bytes_pointer::{PointerAndLength, PointerBytes};
use wcontainers_volatile_bytes_shared_mapping::{MapError, Mapped, SharedMapping};

// The framework takes a vector register's value in a vector register, as C passes a 16-byte vector, and applevisor-sys
// declares it as a `u128`, which Rust passes in two general registers: the call is made in C.
#[expect(
    unsafe_code,
    reason = "declares the C function the build script compiles, whose signature it matches"
)]
unsafe extern "C" {
    fn set_simd_fp_reg_by_vector(
        vcpu: hv_vcpu_t,
        reg: SimdFpReg,
        value: *const u128,
    ) -> hv_return_t;
}

/// Hypervisor.framework, called on this Mac.
pub struct DirectHypervisorFramework;

/// A VM configuration, released on drop.
pub struct HvfVmConfig(hv_vm_config_t);

/// A vCPU configuration, released on drop.
pub struct HvfVcpuConfig(hv_vcpu_config_t);

/// A GIC configuration, released on drop.
pub struct HvfGicConfig(hv_gic_config_t);

/// A saved GIC state, released on drop.
pub struct HvfGicState(hv_gic_state_t);

/// The process's VM, which keeps every memory mapped into it while it or a vCPU of it lives.
pub struct HvfVm(Arc<Machine>);

/// A vCPU, used and ended only on the thread that made it.
pub struct HvfVcpu {
    id: hv_vcpu_t,
    exit: PointerBytes,
    _machine: Arc<Machine>,
    /// Neither `Send` nor `Sync`: the framework binds a vCPU to the thread that made it.
    _thread: PhantomData<*const ()>,
}

/// The process's VM, destroyed once nothing of it lives, and a view of each memory mapped into it with its address.
struct Machine {
    mapped: Mutex<Vec<(u64, SharedMapping)>>,
}

/// Returns the framework's return code as a result.
fn done(code: hv_return_t) -> Result<(), Failure> {
    if code == hv_error_t::HV_SUCCESS as hv_return_t {
        Ok(())
    } else {
        Err(code)
    }
}

/// Returns the framework's code for a call it refused for a bad argument.
fn bad_argument() -> Failure {
    hv_error_t::HV_BAD_ARGUMENT as hv_return_t
}

/// Returns the framework's code for a call it refused for want of resources.
fn no_resources() -> Failure {
    hv_error_t::HV_NO_RESOURCES as hv_return_t
}

/// Returns the granule a raw value names, read where the framework writes a granule.
fn granule(raw: u32) -> Result<IpaGranule, Failure> {
    [
        hv_ipa_granule_t::HV_IPA_GRANULE_4KB,
        hv_ipa_granule_t::HV_IPA_GRANULE_16KB,
    ]
    .into_iter()
    .find(|granule| *granule as u32 == raw)
    .ok_or(hv_error_t::HV_ERROR as hv_return_t)
}

/// Returns the 8 bytes at the offset of a buffer, as a little-endian number.
fn word(bytes: &[u8], at: usize) -> u64 {
    let mut word = [u8::MIN; size_of::<u64>()];
    word.copy_from_slice(&bytes[at..at + size_of::<u64>()]);
    u64::from_le_bytes(word)
}

impl Drop for HvfVmConfig {
    fn drop(&mut self) {
        #[expect(
            unsafe_code,
            reason = "releases the configuration object this value alone owns"
        )]
        unsafe {
            os_release(self.0)
        };
    }
}

impl Drop for HvfVcpuConfig {
    fn drop(&mut self) {
        #[expect(
            unsafe_code,
            reason = "releases the configuration object this value alone owns"
        )]
        unsafe {
            os_release(self.0)
        };
    }
}

impl Drop for HvfGicConfig {
    fn drop(&mut self) {
        #[expect(
            unsafe_code,
            reason = "releases the configuration object this value alone owns"
        )]
        unsafe {
            os_release(self.0)
        };
    }
}

impl Drop for HvfGicState {
    fn drop(&mut self) {
        #[expect(
            unsafe_code,
            reason = "releases the state object this value alone owns"
        )]
        unsafe {
            os_release(self.0)
        };
    }
}

/// Destroys the VM once its handle and every vCPU are gone, before the memory views it kept are dropped.
impl Drop for Machine {
    fn drop(&mut self) {
        #[expect(
            unsafe_code,
            reason = "destroys the process's VM, which no handle or vCPU reaches any more"
        )]
        unsafe {
            hv_vm_destroy()
        };
    }
}

/// Destroys the vCPU, on the thread that made it, since the type cannot leave that thread.
impl Drop for HvfVcpu {
    fn drop(&mut self) {
        #[expect(
            unsafe_code,
            reason = "destroys a live vCPU on the thread that made it"
        )]
        unsafe {
            hv_vcpu_destroy(self.id)
        };
    }
}

impl HypervisorFramework for DirectHypervisorFramework {
    type VmConfig = HvfVmConfig;
    type VcpuConfig = HvfVcpuConfig;
    type GicConfig = HvfGicConfig;
    type GicState = HvfGicState;
    type Vm = HvfVm;
    type Vcpu = HvfVcpu;
    type Memory = SharedMapping;

    fn vm_config_create(&self) -> Result<HvfVmConfig, Failure> {
        #[expect(
            unsafe_code,
            reason = "hv_vm_config_create takes nothing and returns a new object, or none"
        )]
        let config = unsafe { hv_vm_config_create() };
        if config.is_null() {
            Err(no_resources())
        } else {
            Ok(HvfVmConfig(config))
        }
    }

    fn vm_config_get_default_ipa_granule(&self) -> Result<IpaGranule, Failure> {
        let mut raw = u32::MIN;
        #[expect(
            unsafe_code,
            reason = "hv_vm_config_get_default_ipa_granule writes one 32-bit enum to a live u32"
        )]
        let code = unsafe { hv_vm_config_get_default_ipa_granule((&raw mut raw).cast()) };
        done(code).and_then(|()| granule(raw))
    }

    fn vm_config_get_ipa_granule(&self, config: &HvfVmConfig) -> Result<IpaGranule, Failure> {
        let mut raw = u32::MIN;
        #[expect(
            unsafe_code,
            reason = "hv_vm_config_get_ipa_granule reads a live configuration and writes one 32-bit enum to a live u32"
        )]
        let code = unsafe { hv_vm_config_get_ipa_granule(config.0, (&raw mut raw).cast()) };
        done(code).and_then(|()| granule(raw))
    }

    fn vm_create(&self, config: Option<&HvfVmConfig>) -> Result<HvfVm, Failure> {
        #[expect(
            unsafe_code,
            reason = "hv_vm_create reads a live configuration, or none"
        )]
        let code = unsafe { hv_vm_create(config.map_or(ptr::null_mut(), |config| config.0)) };
        done(code)?;
        Ok(HvfVm(Arc::new(Machine {
            mapped: Mutex::new(Vec::new()),
        })))
    }

    fn create_user_memory(&self, length: usize) -> Result<SharedMapping, Failure> {
        let mapped = Mapped::anonymous(length).map_err(|error| match error {
            MapError::Refused(_) => no_resources(),
            _ => bad_argument(),
        })?;
        Ok(SharedMapping::new(mapped))
    }

    fn vm_map(
        &self,
        vm: &HvfVm,
        memory: &SharedMapping,
        ipa: u64,
        flags: MemoryFlags,
    ) -> Result<(), Failure> {
        let mut mapped = vm.0.mapped.lock().unwrap_or_else(PoisonError::into_inner);
        let kept = memory
            .part(u64::MIN..memory.len())
            .ok_or_else(bad_argument)?;
        let length = usize::try_from(kept.len()).map_err(|_| bad_argument())?;
        #[expect(
            unsafe_code,
            reason = "hv_vm_map keeps the host addresses, whose view the VM keeps until a vm_unmap covers them"
        )]
        let code = unsafe { hv_vm_map(kept.address() as *const c_void, ipa, length, flags) };
        done(code)?;
        mapped.push((ipa, kept));
        Ok(())
    }

    /// Drops the views of memory the range wholly covers; a view the range covers only in part stays kept.
    fn vm_unmap(&self, vm: &HvfVm, ipa: u64, size: usize) -> Result<(), Failure> {
        let mut mapped = vm.0.mapped.lock().unwrap_or_else(PoisonError::into_inner);
        #[expect(
            unsafe_code,
            reason = "hv_vm_unmap only takes guest addresses away from the VM"
        )]
        let code = unsafe { hv_vm_unmap(ipa, size) };
        done(code)?;
        let end = ipa.saturating_add(size as u64);
        mapped.retain(|(at, memory)| *at < ipa || at.saturating_add(memory.len()) > end);
        Ok(())
    }

    fn vcpu_config_create(&self) -> Result<HvfVcpuConfig, Failure> {
        #[expect(
            unsafe_code,
            reason = "hv_vcpu_config_create takes nothing and returns a new object, or none"
        )]
        let config = unsafe { hv_vcpu_config_create() };
        if config.is_null() {
            Err(no_resources())
        } else {
            Ok(HvfVcpuConfig(config))
        }
    }

    fn vcpu_config_get_ccsidr_el1_sys_reg_values(
        &self,
        config: &HvfVcpuConfig,
        cache_type: CacheType,
    ) -> Result<CacheSizes, Failure> {
        let mut values = [u64::MIN; CACHE_LEVELS];
        #[expect(
            unsafe_code,
            reason = "hv_vcpu_config_get_ccsidr_el1_sys_reg_values writes the array of 8 its header declares"
        )]
        let code = unsafe {
            hv_vcpu_config_get_ccsidr_el1_sys_reg_values(config.0, cache_type, values.as_mut_ptr())
        };
        done(code).map(|()| values)
    }

    fn vcpu_create(&self, vm: &HvfVm, config: Option<&HvfVcpuConfig>) -> Result<HvfVcpu, Failure> {
        let mut id: hv_vcpu_t = hv_vcpu_t::MIN;
        let mut exit: *const hv_vcpu_exit_t = ptr::null();
        #[expect(
            unsafe_code,
            reason = "hv_vcpu_create writes the new vCPU's id and its exit record's address to live locals"
        )]
        let code = unsafe {
            hv_vcpu_create(
                &mut id,
                &mut exit,
                config.map_or(ptr::null_mut(), |config| config.0),
            )
        };
        done(code)?;
        if exit.is_null() {
            return Err(hv_error_t::HV_ERROR as hv_return_t);
        }
        #[expect(
            unsafe_code,
            reason = "the framework keeps a vCPU's exit record while the vCPU lives, and writes it only during a run on its thread"
        )]
        let pointer =
            unsafe { PointerAndLength::new(exit.cast_mut().cast(), size_of::<hv_vcpu_exit_t>()) };
        Ok(HvfVcpu {
            id,
            exit: PointerBytes::new(pointer),
            _machine: Arc::clone(&vm.0),
            _thread: PhantomData,
        })
    }

    fn vcpu_run(&self, vcpu: &HvfVcpu) -> Result<(), Failure> {
        #[expect(
            unsafe_code,
            reason = "hv_vcpu_run runs a live vCPU on the thread that made it"
        )]
        let code = unsafe { hv_vcpu_run(vcpu.id) };
        done(code)
    }

    /// Reads the exit record as bytes and decodes it, so that an exit reason the framework adds later reads as `UNKNOWN`.
    fn vcpu_exit(&self, vcpu: &HvfVcpu) -> VcpuExit {
        let mut bytes = [u8::MIN; size_of::<hv_vcpu_exit_t>()];
        vcpu.exit.read(&mut bytes);
        let reason_at = offset_of!(hv_vcpu_exit_t, reason);
        let mut raw = [u8::MIN; size_of::<u32>()];
        raw.copy_from_slice(&bytes[reason_at..reason_at + size_of::<u32>()]);
        let raw = u32::from_le_bytes(raw);
        let reasons = [
            hv_exit_reason_t::CANCELED,
            hv_exit_reason_t::EXCEPTION,
            hv_exit_reason_t::VTIMER_ACTIVATED,
            hv_exit_reason_t::UNKNOWN,
        ];
        let reason = reasons
            .into_iter()
            .find(|reason| *reason as u32 == raw)
            .unwrap_or(hv_exit_reason_t::UNKNOWN);
        let exception = offset_of!(hv_vcpu_exit_t, exception);
        VcpuExit {
            reason,
            exception: hv_vcpu_exit_exception_t {
                syndrome: word(
                    &bytes,
                    exception + offset_of!(hv_vcpu_exit_exception_t, syndrome),
                ),
                virtual_address: word(
                    &bytes,
                    exception + offset_of!(hv_vcpu_exit_exception_t, virtual_address),
                ),
                physical_address: word(
                    &bytes,
                    exception + offset_of!(hv_vcpu_exit_exception_t, physical_address),
                ),
            },
        }
    }

    fn vcpu_id(&self, vcpu: &HvfVcpu) -> VcpuId {
        vcpu.id
    }

    fn vcpus_exit(&self, vcpus: &[VcpuId]) -> Result<(), Failure> {
        let count = u32::try_from(vcpus.len()).map_err(|_| bad_argument())?;
        #[expect(
            unsafe_code,
            reason = "hv_vcpus_exit reads the live slice's ids; a stale id names no vCPU or makes another of this process exit"
        )]
        let code = unsafe { hv_vcpus_exit(vcpus.as_ptr(), count) };
        done(code)
    }

    fn vcpu_get_simd_fp_reg(&self, vcpu: &HvfVcpu, reg: SimdFpReg) -> Result<u128, Failure> {
        let mut value = u128::MIN;
        #[expect(
            unsafe_code,
            reason = "hv_vcpu_get_simd_fp_reg on a live vCPU, on its thread, writes 16 bytes to a live u128"
        )]
        let code = unsafe { hv_vcpu_get_simd_fp_reg(vcpu.id, reg, &mut value) };
        done(code).map(|()| value)
    }

    fn vcpu_set_simd_fp_reg(
        &self,
        vcpu: &HvfVcpu,
        reg: SimdFpReg,
        value: u128,
    ) -> Result<(), Failure> {
        #[expect(
            unsafe_code,
            reason = "the C function reads 16 bytes of a live u128 and sets a live vCPU's register, on its thread"
        )]
        let code = unsafe { set_simd_fp_reg_by_vector(vcpu.id, reg, &value) };
        done(code)
    }

    fn vcpu_get_sme_state(&self, vcpu: &HvfVcpu) -> Result<SmeState, Failure> {
        let mut state = SmeState {
            streaming_sve_mode_enabled: false,
            za_storage_enabled: false,
        };
        #[expect(
            unsafe_code,
            reason = "hv_vcpu_get_sme_state on a live vCPU, on its thread, writes one state to a live local"
        )]
        let code = unsafe { hv_vcpu_get_sme_state(vcpu.id, &mut state) };
        done(code).map(|()| state)
    }

    fn vcpu_set_sme_state(&self, vcpu: &HvfVcpu, sme_state: &SmeState) -> Result<(), Failure> {
        #[expect(
            unsafe_code,
            reason = "hv_vcpu_set_sme_state on a live vCPU, on its thread, reads one live state"
        )]
        let code = unsafe { hv_vcpu_set_sme_state(vcpu.id, sme_state) };
        done(code)
    }

    fn vcpu_get_sme_z_reg(
        &self,
        vcpu: &HvfVcpu,
        reg: SmeZReg,
        value: &mut [u8],
    ) -> Result<(), Failure> {
        #[expect(
            unsafe_code,
            reason = "hv_vcpu_get_sme_z_reg writes at most the length it is given, of a live slice"
        )]
        let code = unsafe { hv_vcpu_get_sme_z_reg(vcpu.id, reg, value.as_mut_ptr(), value.len()) };
        done(code)
    }

    fn vcpu_set_sme_z_reg(
        &self,
        vcpu: &HvfVcpu,
        reg: SmeZReg,
        value: &[u8],
    ) -> Result<(), Failure> {
        #[expect(
            unsafe_code,
            reason = "hv_vcpu_set_sme_z_reg reads at most the length it is given, of a live slice"
        )]
        let code = unsafe { hv_vcpu_set_sme_z_reg(vcpu.id, reg, value.as_ptr(), value.len()) };
        done(code)
    }

    fn vcpu_get_sme_p_reg(
        &self,
        vcpu: &HvfVcpu,
        reg: SmePReg,
        value: &mut [u8],
    ) -> Result<(), Failure> {
        #[expect(
            unsafe_code,
            reason = "hv_vcpu_get_sme_p_reg writes at most the length it is given, of a live slice"
        )]
        let code = unsafe { hv_vcpu_get_sme_p_reg(vcpu.id, reg, value.as_mut_ptr(), value.len()) };
        done(code)
    }

    fn vcpu_set_sme_p_reg(
        &self,
        vcpu: &HvfVcpu,
        reg: SmePReg,
        value: &[u8],
    ) -> Result<(), Failure> {
        #[expect(
            unsafe_code,
            reason = "hv_vcpu_set_sme_p_reg reads at most the length it is given, of a live slice"
        )]
        let code = unsafe { hv_vcpu_set_sme_p_reg(vcpu.id, reg, value.as_ptr(), value.len()) };
        done(code)
    }

    fn vcpu_get_sme_za_reg(&self, vcpu: &HvfVcpu, value: &mut [u8]) -> Result<(), Failure> {
        #[expect(
            unsafe_code,
            reason = "hv_vcpu_get_sme_za_reg writes at most the length it is given, of a live slice"
        )]
        let code = unsafe { hv_vcpu_get_sme_za_reg(vcpu.id, value.as_mut_ptr(), value.len()) };
        done(code)
    }

    fn vcpu_set_sme_za_reg(&self, vcpu: &HvfVcpu, value: &[u8]) -> Result<(), Failure> {
        #[expect(
            unsafe_code,
            reason = "hv_vcpu_set_sme_za_reg reads at most the length it is given, of a live slice"
        )]
        let code = unsafe { hv_vcpu_set_sme_za_reg(vcpu.id, value.as_ptr(), value.len()) };
        done(code)
    }

    fn vcpu_get_sme_zt0_reg(&self, vcpu: &HvfVcpu) -> Result<SmeZt0, Failure> {
        let mut value: SmeZt0 = [u8::MIN; size_of::<SmeZt0>()];
        #[expect(
            unsafe_code,
            reason = "hv_vcpu_get_sme_zt0_reg writes the 64 bytes of ZT0 to a live array of 64"
        )]
        let code = unsafe { hv_vcpu_get_sme_zt0_reg(vcpu.id, value.as_mut_ptr()) };
        done(code).map(|()| value)
    }

    fn vcpu_set_sme_zt0_reg(&self, vcpu: &HvfVcpu, value: &SmeZt0) -> Result<(), Failure> {
        #[expect(
            unsafe_code,
            reason = "hv_vcpu_set_sme_zt0_reg reads the 64 bytes of a live array of 64"
        )]
        let code = unsafe { hv_vcpu_set_sme_zt0_reg(vcpu.id, value.as_ptr()) };
        done(code)
    }

    fn gic_config_create(&self) -> Result<HvfGicConfig, Failure> {
        #[expect(
            unsafe_code,
            reason = "hv_gic_config_create takes nothing and returns a new object, or none"
        )]
        let config = unsafe { hv_gic_config_create() };
        if config.is_null() {
            Err(no_resources())
        } else {
            Ok(HvfGicConfig(config))
        }
    }

    fn gic_create(&self, _vm: &HvfVm, config: &HvfGicConfig) -> Result<(), Failure> {
        #[expect(
            unsafe_code,
            reason = "hv_gic_create reads a live configuration, for the live VM"
        )]
        let code = unsafe { hv_gic_create(config.0) };
        done(code)
    }

    fn gic_set_state(&self, _vm: &HvfVm, gic_state_data: &[u8]) -> Result<(), Failure> {
        #[expect(
            unsafe_code,
            reason = "hv_gic_set_state reads the length it is given of a live slice"
        )]
        let code =
            unsafe { hv_gic_set_state(gic_state_data.as_ptr().cast(), gic_state_data.len()) };
        done(code)
    }

    fn gic_get_spi_interrupt_range(&self) -> Result<(u32, u32), Failure> {
        let (mut base, mut count) = (u32::MIN, u32::MIN);
        #[expect(
            unsafe_code,
            reason = "hv_gic_get_spi_interrupt_range writes one u32 to each of two live locals"
        )]
        let code = unsafe { hv_gic_get_spi_interrupt_range(&mut base, &mut count) };
        done(code).map(|()| (base, count))
    }

    fn gic_get_intid(&self, interrupt: GicIntId) -> Result<u32, Failure> {
        let mut intid = u32::MIN;
        #[expect(
            unsafe_code,
            reason = "hv_gic_get_intid writes one u32 to a live local"
        )]
        let code = unsafe { hv_gic_get_intid(interrupt, &mut intid) };
        done(code).map(|()| intid)
    }

    fn gic_state_create(&self, _vm: &HvfVm) -> Result<HvfGicState, Failure> {
        #[expect(
            unsafe_code,
            reason = "hv_gic_state_create takes nothing and returns a new object of the live VM's GIC, or none"
        )]
        let state = unsafe { hv_gic_state_create() };
        if state.is_null() {
            Err(no_resources())
        } else {
            Ok(HvfGicState(state))
        }
    }

    fn gic_state_get_size(&self, state: &HvfGicState) -> Result<usize, Failure> {
        let mut size = usize::MIN;
        #[expect(
            unsafe_code,
            reason = "hv_gic_state_get_size reads a live state and writes one usize to a live local"
        )]
        let code = unsafe { hv_gic_state_get_size(state.0, &mut size) };
        done(code).map(|()| size)
    }

    /// Asks the state's length first, so that the buffer holds every byte the framework writes.
    fn gic_state_get_data(&self, state: &HvfGicState) -> Result<Vec<u8>, Failure> {
        let mut data = vec![u8::MIN; self.gic_state_get_size(state)?];
        #[expect(
            unsafe_code,
            reason = "hv_gic_state_get_data writes the state's length, which the live buffer holds"
        )]
        let code = unsafe { hv_gic_state_get_data(state.0, data.as_mut_ptr().cast()) };
        done(code).map(|()| data)
    }

    fn vm_config_get_max_ipa_size(&self) -> Result<u32, Failure> {
        let mut value = u32::MIN;
        #[expect(
            unsafe_code,
            reason = "hv_vm_config_get_max_ipa_size writes one u32 to a live local"
        )]
        let code = unsafe { hv_vm_config_get_max_ipa_size(&mut value) };
        done(code).map(|()| value)
    }
    fn vm_config_get_default_ipa_size(&self) -> Result<u32, Failure> {
        let mut value = u32::MIN;
        #[expect(
            unsafe_code,
            reason = "hv_vm_config_get_default_ipa_size writes one u32 to a live local"
        )]
        let code = unsafe { hv_vm_config_get_default_ipa_size(&mut value) };
        done(code).map(|()| value)
    }
    fn vm_config_get_el2_supported(&self) -> Result<bool, Failure> {
        let mut value = false;
        #[expect(
            unsafe_code,
            reason = "hv_vm_config_get_el2_supported writes one bool to a live local"
        )]
        let code = unsafe { hv_vm_config_get_el2_supported(&mut value) };
        done(code).map(|()| value)
    }
    fn vm_get_max_vcpu_count(&self) -> Result<u32, Failure> {
        let mut value = u32::MIN;
        #[expect(
            unsafe_code,
            reason = "hv_vm_get_max_vcpu_count writes one u32 to a live local"
        )]
        let code = unsafe { hv_vm_get_max_vcpu_count(&mut value) };
        done(code).map(|()| value)
    }
    fn sme_config_get_max_svl_bytes(&self) -> Result<usize, Failure> {
        let mut value = usize::MIN;
        #[expect(
            unsafe_code,
            reason = "hv_sme_config_get_max_svl_bytes writes one usize to a live local"
        )]
        let code = unsafe { hv_sme_config_get_max_svl_bytes(&mut value) };
        done(code).map(|()| value)
    }
    fn gic_get_distributor_size(&self) -> Result<usize, Failure> {
        let mut value = usize::MIN;
        #[expect(
            unsafe_code,
            reason = "hv_gic_get_distributor_size writes one usize to a live local"
        )]
        let code = unsafe { hv_gic_get_distributor_size(&mut value) };
        done(code).map(|()| value)
    }
    fn gic_get_distributor_base_alignment(&self) -> Result<usize, Failure> {
        let mut value = usize::MIN;
        #[expect(
            unsafe_code,
            reason = "hv_gic_get_distributor_base_alignment writes one usize to a live local"
        )]
        let code = unsafe { hv_gic_get_distributor_base_alignment(&mut value) };
        done(code).map(|()| value)
    }
    fn gic_get_redistributor_region_size(&self) -> Result<usize, Failure> {
        let mut value = usize::MIN;
        #[expect(
            unsafe_code,
            reason = "hv_gic_get_redistributor_region_size writes one usize to a live local"
        )]
        let code = unsafe { hv_gic_get_redistributor_region_size(&mut value) };
        done(code).map(|()| value)
    }
    fn gic_get_redistributor_size(&self) -> Result<usize, Failure> {
        let mut value = usize::MIN;
        #[expect(
            unsafe_code,
            reason = "hv_gic_get_redistributor_size writes one usize to a live local"
        )]
        let code = unsafe { hv_gic_get_redistributor_size(&mut value) };
        done(code).map(|()| value)
    }
    fn gic_get_redistributor_base_alignment(&self) -> Result<usize, Failure> {
        let mut value = usize::MIN;
        #[expect(
            unsafe_code,
            reason = "hv_gic_get_redistributor_base_alignment writes one usize to a live local"
        )]
        let code = unsafe { hv_gic_get_redistributor_base_alignment(&mut value) };
        done(code).map(|()| value)
    }
    fn gic_get_msi_region_size(&self) -> Result<usize, Failure> {
        let mut value = usize::MIN;
        #[expect(
            unsafe_code,
            reason = "hv_gic_get_msi_region_size writes one usize to a live local"
        )]
        let code = unsafe { hv_gic_get_msi_region_size(&mut value) };
        done(code).map(|()| value)
    }
    fn gic_get_msi_region_base_alignment(&self) -> Result<usize, Failure> {
        let mut value = usize::MIN;
        #[expect(
            unsafe_code,
            reason = "hv_gic_get_msi_region_base_alignment writes one usize to a live local"
        )]
        let code = unsafe { hv_gic_get_msi_region_base_alignment(&mut value) };
        done(code).map(|()| value)
    }
    fn vm_config_get_ipa_size(&self, config: &HvfVmConfig) -> Result<u32, Failure> {
        let mut value = u32::MIN;
        #[expect(
            unsafe_code,
            reason = "hv_vm_config_get_ipa_size reads a live configuration and writes one u32 to a live local"
        )]
        let code = unsafe { hv_vm_config_get_ipa_size(config.0, &mut value) };
        done(code).map(|()| value)
    }
    fn vm_config_get_el2_enabled(&self, config: &HvfVmConfig) -> Result<bool, Failure> {
        let mut value = false;
        #[expect(
            unsafe_code,
            reason = "hv_vm_config_get_el2_enabled reads a live configuration and writes one bool to a live local"
        )]
        let code = unsafe { hv_vm_config_get_el2_enabled(config.0, &mut value) };
        done(code).map(|()| value)
    }
    fn vcpu_config_get_feature_reg(
        &self,
        config: &HvfVcpuConfig,
        feature_reg: FeatureReg,
    ) -> Result<u64, Failure> {
        let mut value = u64::MIN;
        #[expect(
            unsafe_code,
            reason = "hv_vcpu_config_get_feature_reg reads a live configuration and writes one u64 to a live local"
        )]
        let code = unsafe { hv_vcpu_config_get_feature_reg(config.0, feature_reg, &mut value) };
        done(code).map(|()| value)
    }
    fn vm_config_set_ipa_size(
        &self,
        config: &mut HvfVmConfig,
        ipa_bit_length: u32,
    ) -> Result<(), Failure> {
        #[expect(
            unsafe_code,
            reason = "hv_vm_config_set_ipa_size changes a live configuration this value owns"
        )]
        let code = unsafe { hv_vm_config_set_ipa_size(config.0, ipa_bit_length) };
        done(code)
    }
    fn vm_config_set_el2_enabled(
        &self,
        config: &mut HvfVmConfig,
        el2_enabled: bool,
    ) -> Result<(), Failure> {
        #[expect(
            unsafe_code,
            reason = "hv_vm_config_set_el2_enabled changes a live configuration this value owns"
        )]
        let code = unsafe { hv_vm_config_set_el2_enabled(config.0, el2_enabled) };
        done(code)
    }
    fn vm_config_set_ipa_granule(
        &self,
        config: &mut HvfVmConfig,
        granule: IpaGranule,
    ) -> Result<(), Failure> {
        #[expect(
            unsafe_code,
            reason = "hv_vm_config_set_ipa_granule changes a live configuration this value owns"
        )]
        let code = unsafe { hv_vm_config_set_ipa_granule(config.0, granule) };
        done(code)
    }
    fn gic_config_set_distributor_base(
        &self,
        config: &mut HvfGicConfig,
        base: u64,
    ) -> Result<(), Failure> {
        #[expect(
            unsafe_code,
            reason = "hv_gic_config_set_distributor_base changes a live configuration this value owns"
        )]
        let code = unsafe { hv_gic_config_set_distributor_base(config.0, base) };
        done(code)
    }
    fn gic_config_set_redistributor_base(
        &self,
        config: &mut HvfGicConfig,
        base: u64,
    ) -> Result<(), Failure> {
        #[expect(
            unsafe_code,
            reason = "hv_gic_config_set_redistributor_base changes a live configuration this value owns"
        )]
        let code = unsafe { hv_gic_config_set_redistributor_base(config.0, base) };
        done(code)
    }
    fn gic_config_set_msi_region_base(
        &self,
        config: &mut HvfGicConfig,
        base: u64,
    ) -> Result<(), Failure> {
        #[expect(
            unsafe_code,
            reason = "hv_gic_config_set_msi_region_base changes a live configuration this value owns"
        )]
        let code = unsafe { hv_gic_config_set_msi_region_base(config.0, base) };
        done(code)
    }
    fn gic_config_set_msi_interrupt_range(
        &self,
        config: &mut HvfGicConfig,
        msi_intid_base: u32,
        msi_intid_count: u32,
    ) -> Result<(), Failure> {
        #[expect(
            unsafe_code,
            reason = "hv_gic_config_set_msi_interrupt_range changes a live configuration this value owns"
        )]
        let code = unsafe {
            hv_gic_config_set_msi_interrupt_range(config.0, msi_intid_base, msi_intid_count)
        };
        done(code)
    }
    fn vcpu_get_pending_interrupt(
        &self,
        vcpu: &HvfVcpu,
        interrupt: InterruptType,
    ) -> Result<bool, Failure> {
        let mut value = false;
        #[expect(
            unsafe_code,
            reason = "hv_vcpu_get_pending_interrupt on a live vCPU, on the thread that made it, writes one bool to a live local"
        )]
        let code = unsafe { hv_vcpu_get_pending_interrupt(vcpu.id, interrupt, &mut value) };
        done(code).map(|()| value)
    }
    fn vcpu_get_exec_time(&self, vcpu: &HvfVcpu) -> Result<u64, Failure> {
        let mut value = u64::MIN;
        #[expect(
            unsafe_code,
            reason = "hv_vcpu_get_exec_time on a live vCPU, on the thread that made it, writes one u64 to a live local"
        )]
        let code = unsafe { hv_vcpu_get_exec_time(vcpu.id, &mut value) };
        done(code).map(|()| value)
    }
    fn vcpu_get_reg(&self, vcpu: &HvfVcpu, reg: Reg) -> Result<u64, Failure> {
        let mut value = u64::MIN;
        #[expect(
            unsafe_code,
            reason = "hv_vcpu_get_reg on a live vCPU, on the thread that made it, writes one u64 to a live local"
        )]
        let code = unsafe { hv_vcpu_get_reg(vcpu.id, reg, &mut value) };
        done(code).map(|()| value)
    }
    fn vcpu_get_sys_reg(&self, vcpu: &HvfVcpu, reg: SysReg) -> Result<u64, Failure> {
        let mut value = u64::MIN;
        #[expect(
            unsafe_code,
            reason = "hv_vcpu_get_sys_reg on a live vCPU, on the thread that made it, writes one u64 to a live local"
        )]
        let code = unsafe { hv_vcpu_get_sys_reg(vcpu.id, reg, &mut value) };
        done(code).map(|()| value)
    }
    fn vcpu_get_trap_debug_exceptions(&self, vcpu: &HvfVcpu) -> Result<bool, Failure> {
        let mut value = false;
        #[expect(
            unsafe_code,
            reason = "hv_vcpu_get_trap_debug_exceptions on a live vCPU, on the thread that made it, writes one bool to a live local"
        )]
        let code = unsafe { hv_vcpu_get_trap_debug_exceptions(vcpu.id, &mut value) };
        done(code).map(|()| value)
    }
    fn vcpu_get_trap_debug_reg_accesses(&self, vcpu: &HvfVcpu) -> Result<bool, Failure> {
        let mut value = false;
        #[expect(
            unsafe_code,
            reason = "hv_vcpu_get_trap_debug_reg_accesses on a live vCPU, on the thread that made it, writes one bool to a live local"
        )]
        let code = unsafe { hv_vcpu_get_trap_debug_reg_accesses(vcpu.id, &mut value) };
        done(code).map(|()| value)
    }
    fn vcpu_get_vtimer_mask(&self, vcpu: &HvfVcpu) -> Result<bool, Failure> {
        let mut value = false;
        #[expect(
            unsafe_code,
            reason = "hv_vcpu_get_vtimer_mask on a live vCPU, on the thread that made it, writes one bool to a live local"
        )]
        let code = unsafe { hv_vcpu_get_vtimer_mask(vcpu.id, &mut value) };
        done(code).map(|()| value)
    }
    fn vcpu_get_vtimer_offset(&self, vcpu: &HvfVcpu) -> Result<u64, Failure> {
        let mut value = u64::MIN;
        #[expect(
            unsafe_code,
            reason = "hv_vcpu_get_vtimer_offset on a live vCPU, on the thread that made it, writes one u64 to a live local"
        )]
        let code = unsafe { hv_vcpu_get_vtimer_offset(vcpu.id, &mut value) };
        done(code).map(|()| value)
    }
    fn gic_get_redistributor_base(&self, vcpu: &HvfVcpu) -> Result<u64, Failure> {
        let mut value = u64::MIN;
        #[expect(
            unsafe_code,
            reason = "hv_gic_get_redistributor_base on a live vCPU, on the thread that made it, writes one u64 to a live local"
        )]
        let code = unsafe { hv_gic_get_redistributor_base(vcpu.id, &mut value) };
        done(code).map(|()| value)
    }
    fn gic_get_redistributor_reg(
        &self,
        vcpu: &HvfVcpu,
        reg: GicRedistributorReg,
    ) -> Result<u64, Failure> {
        let mut value = u64::MIN;
        #[expect(
            unsafe_code,
            reason = "hv_gic_get_redistributor_reg on a live vCPU, on the thread that made it, writes one u64 to a live local"
        )]
        let code = unsafe { hv_gic_get_redistributor_reg(vcpu.id, reg, &mut value) };
        done(code).map(|()| value)
    }
    fn gic_get_icc_reg(&self, vcpu: &HvfVcpu, reg: GicIccReg) -> Result<u64, Failure> {
        let mut value = u64::MIN;
        #[expect(
            unsafe_code,
            reason = "hv_gic_get_icc_reg on a live vCPU, on the thread that made it, writes one u64 to a live local"
        )]
        let code = unsafe { hv_gic_get_icc_reg(vcpu.id, reg, &mut value) };
        done(code).map(|()| value)
    }
    fn gic_get_ich_reg(&self, vcpu: &HvfVcpu, reg: GicIchReg) -> Result<u64, Failure> {
        let mut value = u64::MIN;
        #[expect(
            unsafe_code,
            reason = "hv_gic_get_ich_reg on a live vCPU, on the thread that made it, writes one u64 to a live local"
        )]
        let code = unsafe { hv_gic_get_ich_reg(vcpu.id, reg, &mut value) };
        done(code).map(|()| value)
    }
    fn gic_get_icv_reg(&self, vcpu: &HvfVcpu, reg: GicIcvReg) -> Result<u64, Failure> {
        let mut value = u64::MIN;
        #[expect(
            unsafe_code,
            reason = "hv_gic_get_icv_reg on a live vCPU, on the thread that made it, writes one u64 to a live local"
        )]
        let code = unsafe { hv_gic_get_icv_reg(vcpu.id, reg, &mut value) };
        done(code).map(|()| value)
    }
    fn vcpu_set_pending_interrupt(
        &self,
        vcpu: &HvfVcpu,
        interrupt: InterruptType,
        pending: bool,
    ) -> Result<(), Failure> {
        #[expect(
            unsafe_code,
            reason = "hv_vcpu_set_pending_interrupt on a live vCPU, on the thread that made it, with plain values"
        )]
        let code = unsafe { hv_vcpu_set_pending_interrupt(vcpu.id, interrupt, pending) };
        done(code)
    }
    fn vcpu_set_reg(&self, vcpu: &HvfVcpu, reg: Reg, value: u64) -> Result<(), Failure> {
        #[expect(
            unsafe_code,
            reason = "hv_vcpu_set_reg on a live vCPU, on the thread that made it, with plain values"
        )]
        let code = unsafe { hv_vcpu_set_reg(vcpu.id, reg, value) };
        done(code)
    }
    fn vcpu_set_sys_reg(&self, vcpu: &HvfVcpu, reg: SysReg, value: u64) -> Result<(), Failure> {
        #[expect(
            unsafe_code,
            reason = "hv_vcpu_set_sys_reg on a live vCPU, on the thread that made it, with plain values"
        )]
        let code = unsafe { hv_vcpu_set_sys_reg(vcpu.id, reg, value) };
        done(code)
    }
    fn vcpu_set_trap_debug_exceptions(&self, vcpu: &HvfVcpu, value: bool) -> Result<(), Failure> {
        #[expect(
            unsafe_code,
            reason = "hv_vcpu_set_trap_debug_exceptions on a live vCPU, on the thread that made it, with plain values"
        )]
        let code = unsafe { hv_vcpu_set_trap_debug_exceptions(vcpu.id, value) };
        done(code)
    }
    fn vcpu_set_trap_debug_reg_accesses(&self, vcpu: &HvfVcpu, value: bool) -> Result<(), Failure> {
        #[expect(
            unsafe_code,
            reason = "hv_vcpu_set_trap_debug_reg_accesses on a live vCPU, on the thread that made it, with plain values"
        )]
        let code = unsafe { hv_vcpu_set_trap_debug_reg_accesses(vcpu.id, value) };
        done(code)
    }
    fn vcpu_set_vtimer_mask(&self, vcpu: &HvfVcpu, vtimer_is_masked: bool) -> Result<(), Failure> {
        #[expect(
            unsafe_code,
            reason = "hv_vcpu_set_vtimer_mask on a live vCPU, on the thread that made it, with plain values"
        )]
        let code = unsafe { hv_vcpu_set_vtimer_mask(vcpu.id, vtimer_is_masked) };
        done(code)
    }
    fn vcpu_set_vtimer_offset(&self, vcpu: &HvfVcpu, vtimer_offset: u64) -> Result<(), Failure> {
        #[expect(
            unsafe_code,
            reason = "hv_vcpu_set_vtimer_offset on a live vCPU, on the thread that made it, with plain values"
        )]
        let code = unsafe { hv_vcpu_set_vtimer_offset(vcpu.id, vtimer_offset) };
        done(code)
    }
    fn gic_set_redistributor_reg(
        &self,
        vcpu: &HvfVcpu,
        reg: GicRedistributorReg,
        value: u64,
    ) -> Result<(), Failure> {
        #[expect(
            unsafe_code,
            reason = "hv_gic_set_redistributor_reg on a live vCPU, on the thread that made it, with plain values"
        )]
        let code = unsafe { hv_gic_set_redistributor_reg(vcpu.id, reg, value) };
        done(code)
    }
    fn gic_set_icc_reg(&self, vcpu: &HvfVcpu, reg: GicIccReg, value: u64) -> Result<(), Failure> {
        #[expect(
            unsafe_code,
            reason = "hv_gic_set_icc_reg on a live vCPU, on the thread that made it, with plain values"
        )]
        let code = unsafe { hv_gic_set_icc_reg(vcpu.id, reg, value) };
        done(code)
    }
    fn gic_set_ich_reg(&self, vcpu: &HvfVcpu, reg: GicIchReg, value: u64) -> Result<(), Failure> {
        #[expect(
            unsafe_code,
            reason = "hv_gic_set_ich_reg on a live vCPU, on the thread that made it, with plain values"
        )]
        let code = unsafe { hv_gic_set_ich_reg(vcpu.id, reg, value) };
        done(code)
    }
    fn gic_set_icv_reg(&self, vcpu: &HvfVcpu, reg: GicIcvReg, value: u64) -> Result<(), Failure> {
        #[expect(
            unsafe_code,
            reason = "hv_gic_set_icv_reg on a live vCPU, on the thread that made it, with plain values"
        )]
        let code = unsafe { hv_gic_set_icv_reg(vcpu.id, reg, value) };
        done(code)
    }
    fn gic_set_spi(&self, _vm: &HvfVm, intid: u32, level: bool) -> Result<(), Failure> {
        #[expect(
            unsafe_code,
            reason = "hv_gic_set_spi on the live VM with plain values"
        )]
        let code = unsafe { hv_gic_set_spi(intid, level) };
        done(code)
    }
    fn gic_send_msi(&self, _vm: &HvfVm, address: u64, intid: u32) -> Result<(), Failure> {
        #[expect(
            unsafe_code,
            reason = "hv_gic_send_msi on the live VM with plain values"
        )]
        let code = unsafe { hv_gic_send_msi(address, intid) };
        done(code)
    }
    fn gic_get_distributor_reg(&self, _vm: &HvfVm, reg: GicDistributorReg) -> Result<u64, Failure> {
        let mut value = u64::MIN;
        #[expect(
            unsafe_code,
            reason = "hv_gic_get_distributor_reg on the live VM writes one u64 to a live local"
        )]
        let code = unsafe { hv_gic_get_distributor_reg(reg, &mut value) };
        done(code).map(|()| value)
    }
    fn gic_set_distributor_reg(
        &self,
        _vm: &HvfVm,
        reg: GicDistributorReg,
        value: u64,
    ) -> Result<(), Failure> {
        #[expect(
            unsafe_code,
            reason = "hv_gic_set_distributor_reg on the live VM with plain values"
        )]
        let code = unsafe { hv_gic_set_distributor_reg(reg, value) };
        done(code)
    }
    fn gic_get_msi_reg(&self, _vm: &HvfVm, reg: GicMsiReg) -> Result<u64, Failure> {
        let mut value = u64::MIN;
        #[expect(
            unsafe_code,
            reason = "hv_gic_get_msi_reg on the live VM writes one u64 to a live local"
        )]
        let code = unsafe { hv_gic_get_msi_reg(reg, &mut value) };
        done(code).map(|()| value)
    }
    fn gic_set_msi_reg(&self, _vm: &HvfVm, reg: GicMsiReg, value: u64) -> Result<(), Failure> {
        #[expect(
            unsafe_code,
            reason = "hv_gic_set_msi_reg on the live VM with plain values"
        )]
        let code = unsafe { hv_gic_set_msi_reg(reg, value) };
        done(code)
    }
    fn gic_reset(&self, _vm: &HvfVm) -> Result<(), Failure> {
        #[expect(unsafe_code, reason = "hv_gic_reset on the live VM with plain values")]
        let code = unsafe { hv_gic_reset() };
        done(code)
    }
    fn vm_protect(
        &self,
        _vm: &HvfVm,
        ipa: u64,
        size: usize,
        flags: MemoryFlags,
    ) -> Result<(), Failure> {
        #[expect(unsafe_code, reason = "hv_vm_protect on the live VM with plain values")]
        let code = unsafe { hv_vm_protect(ipa, size, flags) };
        done(code)
    }
}
