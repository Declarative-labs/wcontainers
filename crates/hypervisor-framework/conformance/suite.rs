//! The tests every implementation of wcontainers-hypervisor-framework runs on a Mac, written against its trait alone.
#![cfg(all(target_os = "macos", target_arch = "aarch64"))]

use std::sync::{Mutex, MutexGuard, PoisonError};

use applevisor_sys::{
    HV_MEMORY_EXEC, HV_MEMORY_READ, HV_MEMORY_WRITE, PAGE_SIZE, hv_cache_type_t, hv_error_t,
    hv_exit_reason_t, hv_feature_reg_t, hv_gic_distributor_reg_t, hv_gic_icc_reg_t,
    hv_gic_ich_reg_t, hv_gic_icv_reg_t, hv_gic_intid_t, hv_gic_msi_reg_t,
    hv_gic_redistributor_reg_t, hv_interrupt_type_t, hv_reg_t, hv_return_t, hv_simd_fp_reg_t,
    hv_sme_p_reg_t, hv_sme_z_reg_t, hv_sys_reg_t,
};
use wcontainers_hypervisor_framework::{HypervisorFramework, SmeState, SmeZt0};
use wcontainers_volatile_bytes::VolatileBytes;

/// The framework gives a process one VM at a time, so the tests that make one take turns.
static ONE_VM: Mutex<()> = Mutex::new(());

/// The guest-physical address of the memory each test gives a VM.
///
/// https://developer.apple.com/documentation/hypervisor/hv_vm_map(_:_:_:_:)
const GUEST_MEMORY: u64 = 1 << 32;

/// The guest-physical address the guest stores to last: below the memory and mapped to nothing, so the store exits.
///
/// https://developer.apple.com/documentation/hypervisor/hv_exit_reason_t/exception
const DEVICE: u64 = GUEST_MEMORY / 4;

/// Where in its memory the guest stores, past its two instructions.
const STORED_AT: usize = 2 * size_of::<u64>();

/// The value the guest stores in its memory and then at the device.
const STORED: u64 = u64::MAX >> 1;

/// `str x3, [x4]`: STR (immediate), 64-bit, unsigned offset 0, Rn 4, Rt 3.
///
/// https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/STR--immediate---Store-Register--immediate--
const STORE_TO_MEMORY: u32 = 0xf900_0083;

/// `str x3, [x5]`: STR (immediate), 64-bit, unsigned offset 0, Rn 5, Rt 3.
///
/// https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/STR--immediate---Store-Register--immediate--
const STORE_TO_DEVICE: u32 = 0xf900_00a3;

/// PSTATE at EL1, using SP_EL1, with debug, SError, IRQ and FIQ masked.
///
/// https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/SPSR-EL1--Saved-Program-Status-Register--EL1-
const EL1_MASKED: u64 = 0x3c5;

/// The GIC's distributor, placed above the memory, and its redistributors just past it.
///
/// https://developer.apple.com/documentation/hypervisor/hv_gic_config_set_distributor_base(_:_:)
const DISTRIBUTOR: u64 = 2 * GUEST_MEMORY;

/// The value the vCPU state test writes to registers.
const WRITTEN: u64 = u64::MAX;

/// Waits for the VM's turn, past a test that failed holding it.
fn take_turn() -> MutexGuard<'static, ()> {
    ONE_VM.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Returns the framework's code for a feature this Mac lacks.
fn unsupported() -> hv_return_t {
    hv_error_t::HV_UNSUPPORTED as hv_return_t
}

/// Returns the eight bytes past `STORED_AT` from the start of the memory.
fn stored(memory: &impl VolatileBytes) -> u64 {
    let mut bytes = [u8::MIN; STORED_AT + size_of::<u64>()];
    assert_eq!(memory.read(&mut bytes), bytes.len());
    u64::from_le_bytes(bytes[STORED_AT..].try_into().expect("eight bytes"))
}

/// Checks the limits and defaults of a VM configuration, and the GIC's fixed sizes.
pub fn reads_the_configuration_and_its_limits<H: HypervisorFramework>(framework: H) {
    let mut config = framework.vm_config_create().expect("a VM configuration");
    let largest = framework
        .vm_config_get_max_ipa_size()
        .expect("the largest address size");
    let default = framework
        .vm_config_get_default_ipa_size()
        .expect("the default address size");
    assert!(largest >= default);
    framework
        .vm_config_set_ipa_size(&mut config, default)
        .expect("the default size set");
    assert_eq!(framework.vm_config_get_ipa_size(&config), Ok(default));
    if framework.vm_config_get_el2_supported() == Ok(true) {
        framework
            .vm_config_set_el2_enabled(&mut config, false)
            .expect("EL2 off");
        assert_eq!(framework.vm_config_get_el2_enabled(&config), Ok(false));
    }
    let granule = framework
        .vm_config_get_default_ipa_granule()
        .expect("the default granule");
    framework
        .vm_config_set_ipa_granule(&mut config, granule)
        .expect("the default granule set");
    assert_eq!(framework.vm_config_get_ipa_granule(&config), Ok(granule));
    assert!(framework.vm_get_max_vcpu_count().expect("a count") > u32::MIN);
    let svl = framework.sme_config_get_max_svl_bytes();
    assert!(svl.is_ok() || svl == Err(unsupported()));
    for size in [
        framework.gic_get_distributor_size(),
        framework.gic_get_distributor_base_alignment(),
        framework.gic_get_redistributor_region_size(),
        framework.gic_get_redistributor_size(),
        framework.gic_get_redistributor_base_alignment(),
        framework.gic_get_msi_region_size(),
        framework.gic_get_msi_region_base_alignment(),
    ] {
        assert!(size.expect("a GIC size") > usize::MIN);
    }
    let (_, count) = framework
        .gic_get_spi_interrupt_range()
        .expect("the shared interrupts");
    assert!(count > u32::MIN);
    assert!(
        framework
            .gic_get_intid(hv_gic_intid_t::EL1_VIRTUAL_TIMER)
            .expect("the timer's interrupt")
            > u32::MIN
    );
}

/// Checks that a guest stores to its memory and then exits at an address mapped to nothing, both seen from the host.
pub fn runs_a_guest_that_stores_and_exits<H: HypervisorFramework>(framework: H) {
    let _turn = take_turn();
    let vm = framework.vm_create(None).expect("the VM");
    let memory = framework.create_user_memory(PAGE_SIZE).expect("a page");
    let program: Vec<u8> = [STORE_TO_MEMORY, STORE_TO_DEVICE]
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect();
    assert_eq!(memory.write(&program), program.len());
    framework
        .vm_map(
            &vm,
            &memory,
            GUEST_MEMORY,
            HV_MEMORY_READ | HV_MEMORY_WRITE | HV_MEMORY_EXEC,
        )
        .expect("mapped");
    let config = framework
        .vcpu_config_create()
        .expect("a vCPU configuration");
    assert!(
        framework
            .vcpu_config_get_feature_reg(&config, hv_feature_reg_t::ID_AA64ISAR0_EL1)
            .is_ok()
    );
    assert!(
        framework
            .vcpu_config_get_ccsidr_el1_sys_reg_values(&config, hv_cache_type_t::DATA)
            .is_ok()
    );
    let vcpu = framework.vcpu_create(&vm, Some(&config)).expect("a vCPU");
    for (reg, value) in [
        (hv_reg_t::PC, GUEST_MEMORY),
        (hv_reg_t::CPSR, EL1_MASKED),
        (hv_reg_t::X4, GUEST_MEMORY + STORED_AT as u64),
        (hv_reg_t::X3, STORED),
        (hv_reg_t::X5, DEVICE),
    ] {
        framework
            .vcpu_set_reg(&vcpu, reg, value)
            .expect("a register set");
    }
    framework.vcpu_run(&vcpu).expect("the guest runs");
    let exit = framework.vcpu_exit(&vcpu);
    assert_eq!(
        (exit.reason, exit.exception.physical_address),
        (hv_exit_reason_t::EXCEPTION, DEVICE)
    );
    assert_eq!(
        stored(&memory),
        STORED,
        "the host sees what the guest stored"
    );
    assert!(framework.vcpu_get_reg(&vcpu, hv_reg_t::PC).expect("the PC") > GUEST_MEMORY);
    assert!(framework.vcpu_get_exec_time(&vcpu).is_ok());
    framework
        .vcpus_exit(&[framework.vcpu_id(&vcpu)])
        .expect("an exit asked for");
}

/// Checks that memory is mapped, protected and unmapped, and that a range mapped to nothing is not unmapped.
pub fn maps_protects_and_unmaps_memory<H: HypervisorFramework>(framework: H) {
    let _turn = take_turn();
    let vm = framework.vm_create(None).expect("the VM");
    let memory = framework.create_user_memory(PAGE_SIZE).expect("a page");
    framework
        .vm_map(&vm, &memory, GUEST_MEMORY, HV_MEMORY_READ | HV_MEMORY_WRITE)
        .expect("mapped");
    framework
        .vm_protect(&vm, GUEST_MEMORY, PAGE_SIZE, HV_MEMORY_READ)
        .expect("protected");
    framework
        .vm_unmap(&vm, GUEST_MEMORY, PAGE_SIZE)
        .expect("unmapped");
    assert!(
        framework.vm_unmap(&vm, GUEST_MEMORY, PAGE_SIZE).is_err(),
        "nothing is mapped there now"
    );
}

/// Checks that each register and setting of a vCPU reads back what was set, SME's where the Mac has it.
pub fn reads_back_the_vcpu_state<H: HypervisorFramework>(framework: H) {
    let _turn = take_turn();
    let vm = framework.vm_create(None).expect("the VM");
    let vcpu = framework.vcpu_create(&vm, None).expect("a vCPU");
    framework
        .vcpu_set_pending_interrupt(&vcpu, hv_interrupt_type_t::IRQ, true)
        .expect("pending");
    assert_eq!(
        framework.vcpu_get_pending_interrupt(&vcpu, hv_interrupt_type_t::IRQ),
        Ok(true)
    );
    framework
        .vcpu_set_sys_reg(&vcpu, hv_sys_reg_t::TPIDR_EL0, WRITTEN)
        .expect("set");
    assert_eq!(
        framework.vcpu_get_sys_reg(&vcpu, hv_sys_reg_t::TPIDR_EL0),
        Ok(WRITTEN)
    );
    let vector = u128::from(WRITTEN) << u64::BITS | 1;
    framework
        .vcpu_set_simd_fp_reg(&vcpu, hv_simd_fp_reg_t::Q0, vector)
        .expect("set");
    assert_eq!(
        framework.vcpu_get_simd_fp_reg(&vcpu, hv_simd_fp_reg_t::Q0),
        Ok(vector)
    );
    framework
        .vcpu_set_trap_debug_exceptions(&vcpu, true)
        .expect("set");
    assert_eq!(framework.vcpu_get_trap_debug_exceptions(&vcpu), Ok(true));
    framework
        .vcpu_set_trap_debug_reg_accesses(&vcpu, true)
        .expect("set");
    assert_eq!(framework.vcpu_get_trap_debug_reg_accesses(&vcpu), Ok(true));
    framework.vcpu_set_vtimer_mask(&vcpu, true).expect("set");
    assert_eq!(framework.vcpu_get_vtimer_mask(&vcpu), Ok(true));
    framework
        .vcpu_set_vtimer_offset(&vcpu, WRITTEN)
        .expect("set");
    assert_eq!(framework.vcpu_get_vtimer_offset(&vcpu), Ok(WRITTEN));
    let Ok(length) = framework.sme_config_get_max_svl_bytes() else {
        assert_eq!(
            framework.vcpu_get_sme_state(&vcpu).err(),
            Some(unsupported())
        );
        return;
    };
    let on = SmeState {
        streaming_sve_mode_enabled: true,
        za_storage_enabled: true,
    };
    framework.vcpu_set_sme_state(&vcpu, &on).expect("SME on");
    let state = framework.vcpu_get_sme_state(&vcpu).expect("SME's state");
    assert!(state.streaming_sve_mode_enabled && state.za_storage_enabled);
    let z = vec![u8::MAX; length];
    framework
        .vcpu_set_sme_z_reg(&vcpu, hv_sme_z_reg_t::Z0, &z)
        .expect("set");
    let mut read = vec![u8::MIN; length];
    framework
        .vcpu_get_sme_z_reg(&vcpu, hv_sme_z_reg_t::Z0, &mut read)
        .expect("read");
    assert_eq!(read, z);
    let p = vec![u8::MAX; length / u8::BITS as usize];
    framework
        .vcpu_set_sme_p_reg(&vcpu, hv_sme_p_reg_t::P0, &p)
        .expect("set");
    let mut read = vec![u8::MIN; p.len()];
    framework
        .vcpu_get_sme_p_reg(&vcpu, hv_sme_p_reg_t::P0, &mut read)
        .expect("read");
    assert_eq!(read, p);
    let za = vec![u8::MAX; length * length];
    framework.vcpu_set_sme_za_reg(&vcpu, &za).expect("set");
    let mut read = vec![u8::MIN; za.len()];
    framework
        .vcpu_get_sme_za_reg(&vcpu, &mut read)
        .expect("read");
    assert_eq!(read, za);
    let zt0: SmeZt0 = [u8::MAX; size_of::<SmeZt0>()];
    if framework.vcpu_set_sme_zt0_reg(&vcpu, &zt0).is_ok() {
        assert_eq!(framework.vcpu_get_sme_zt0_reg(&vcpu), Ok(zt0));
    }
}

/// Checks the GIC: its placement, an interrupt raised, each register kind read and written, and its saved state.
pub fn sets_up_and_saves_the_gic<H: HypervisorFramework>(framework: H) {
    let _turn = take_turn();
    let mut config = framework.gic_config_create().expect("a GIC configuration");
    let distributor_size = framework.gic_get_distributor_size().expect("a size") as u64;
    let redistributors = DISTRIBUTOR + distributor_size;
    let region = framework
        .gic_get_redistributor_region_size()
        .expect("a size") as u64;
    let msi = redistributors + region;
    framework
        .gic_config_set_distributor_base(&mut config, DISTRIBUTOR)
        .expect("placed");
    framework
        .gic_config_set_redistributor_base(&mut config, redistributors)
        .expect("placed");
    framework
        .gic_config_set_msi_region_base(&mut config, msi)
        .expect("placed");
    let (first, count) = framework
        .gic_get_spi_interrupt_range()
        .expect("the shared interrupts");
    framework
        .gic_config_set_msi_interrupt_range(&mut config, first, count)
        .expect("given to messages");
    let vm = framework.vm_create(None).expect("the VM");
    framework.gic_create(&vm, &config).expect("the GIC");
    let vcpu = framework.vcpu_create(&vm, None).expect("a vCPU");
    framework.gic_set_spi(&vm, first, true).expect("raised");
    assert!(framework.gic_send_msi(&vm, msi, first).is_ok());
    let control = framework
        .gic_get_distributor_reg(&vm, hv_gic_distributor_reg_t::CTLR)
        .expect("the distributor's control");
    framework
        .gic_set_distributor_reg(&vm, hv_gic_distributor_reg_t::CTLR, control)
        .expect("set back");
    let typer = framework
        .gic_get_msi_reg(&vm, hv_gic_msi_reg_t::TYPER)
        .expect("the message frame's type");
    assert!(
        framework
            .gic_set_msi_reg(&vm, hv_gic_msi_reg_t::TYPER, typer)
            .is_err(),
        "TYPER is read only"
    );
    assert!(
        framework
            .gic_get_redistributor_base(&vcpu)
            .expect("the vCPU's redistributor")
            >= redistributors
    );
    let group = hv_gic_redistributor_reg_t::IGROUPR0;
    let groups = framework
        .gic_get_redistributor_reg(&vcpu, group)
        .expect("its groups");
    framework
        .gic_set_redistributor_reg(&vcpu, group, groups)
        .expect("set back");
    let priority = framework
        .gic_get_icc_reg(&vcpu, hv_gic_icc_reg_t::PMR_EL1)
        .expect("the priority mask");
    framework
        .gic_set_icc_reg(&vcpu, hv_gic_icc_reg_t::PMR_EL1, priority)
        .expect("set back");
    let hcr = framework
        .gic_get_ich_reg(&vcpu, hv_gic_ich_reg_t::HCR_EL2)
        .expect("the hypervisor control");
    framework
        .gic_set_ich_reg(&vcpu, hv_gic_ich_reg_t::HCR_EL2, hcr)
        .expect("set back");
    let virtual_priority = framework
        .gic_get_icv_reg(&vcpu, hv_gic_icv_reg_t::PMR_EL1)
        .expect("the virtual priority mask");
    framework
        .gic_set_icv_reg(&vcpu, hv_gic_icv_reg_t::PMR_EL1, virtual_priority)
        .expect("set back");
    let state = framework.gic_state_create(&vm).expect("a saved state");
    let size = framework.gic_state_get_size(&state).expect("its length");
    let data = framework.gic_state_get_data(&state).expect("its bytes");
    assert_eq!(data.len(), size);
    framework.gic_set_state(&vm, &data).expect("restored");
    framework.gic_reset(&vm).expect("reset");
}

/// Runs each suite function above as its own test on the value an implementation gives, made anew for every test.
#[macro_export]
macro_rules! tests {
    ($value:expr) => {
        #[test]
        fn reads_the_configuration_and_its_limits() {
            $crate::reads_the_configuration_and_its_limits($value);
        }

        #[test]
        fn runs_a_guest_that_stores_and_exits() {
            $crate::runs_a_guest_that_stores_and_exits($value);
        }

        #[test]
        fn maps_protects_and_unmaps_memory() {
            $crate::maps_protects_and_unmaps_memory($value);
        }

        #[test]
        fn reads_back_the_vcpu_state() {
            $crate::reads_back_the_vcpu_state($value);
        }

        #[test]
        fn sets_up_and_saves_the_gic() {
            $crate::sets_up_and_saves_the_gic($value);
        }
    };
}
