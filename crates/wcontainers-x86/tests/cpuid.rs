//! `cpuid`'s designed answers (plan 5.10), read back through raw-cpuid, a parser that knows nothing of ours.

use raw_cpuid::{CacheType, CpuId, CpuIdResult, TopologyType};
use wcontainers_x86::cpuid::{
    BRAND, EXTENDED, FAMILY, L1_KIB, L2_KIB, L3_KIB, LINE_BYTES, MAX_BASIC, MAX_EXTENDED, MODEL,
    STEPPING, TIME_STAMP_HZ, VENDOR, answer,
};

fn cpu(avx: bool) -> CpuId<impl raw_cpuid::CpuIdReader> {
    CpuId::with_cpuid_reader(move |leaf, sub_leaf| {
        let [eax, ebx, ecx, edx] = answer(avx, leaf, sub_leaf);
        CpuIdResult { eax, ebx, ecx, edx }
    })
}

/// The default claims: x86-64-v2 and the rest of plan 5.10's list, nothing of the AVX setting or beyond.
#[test]
fn the_default_claims() {
    let cpu = cpu(false);
    assert_eq!(cpu.get_vendor_info().unwrap().as_str().as_bytes(), VENDOR);
    assert_eq!(cpu.get_processor_brand_string().unwrap().as_str(), BRAND);
    let f = cpu.get_feature_info().unwrap();
    assert_eq!(
        (
            u32::from(f.family_id()),
            u32::from(f.model_id()),
            u32::from(f.stepping_id())
        ),
        (FAMILY, MODEL, STEPPING)
    );
    for (name, has) in [
        ("fpu", f.has_fpu()),
        ("tsc", f.has_tsc()),
        ("cx8", f.has_cmpxchg8b()),
        ("sep", f.has_sysenter_sysexit()),
        ("cmov", f.has_cmov()),
        ("clflush", f.has_clflush()),
        ("mmx", f.has_mmx()),
        ("fxsr", f.has_fxsave_fxstor()),
        ("sse", f.has_sse()),
        ("sse2", f.has_sse2()),
        ("sse3", f.has_sse3()),
        ("ssse3", f.has_ssse3()),
        ("sse4.1", f.has_sse41()),
        ("sse4.2", f.has_sse42()),
        ("cx16", f.has_cmpxchg16b()),
        ("movbe", f.has_movbe()),
        ("popcnt", f.has_popcnt()),
    ] {
        assert!(has, "{name}");
    }
    for (name, has) in [
        ("avx", f.has_avx()),
        ("fma", f.has_fma()),
        ("f16c", f.has_f16c()),
        ("xsave", f.has_xsave()),
        ("osxsave", f.has_oxsave()),
        ("aes", f.has_aesni()),
        ("pclmulqdq", f.has_pclmulqdq()),
        ("rdrand", f.has_rdrand()),
        ("htt", f.has_htt()),
        ("x2apic", f.has_x2apic()),
        ("hypervisor", f.has_hypervisor()),
    ] {
        assert!(!has, "{name}");
    }
    assert_eq!(
        u32::from(f.cflush_cache_line_size()) * u64::BITS / u8::BITS,
        LINE_BYTES
    );
    assert_eq!(
        (f.max_logical_processor_ids(), f.initial_local_apic_id()),
        (1, 0)
    );

    let x = cpu.get_extended_feature_info().unwrap();
    assert!(x.has_fsgsbase() && x.has_bmi1() && x.has_adx());
    assert!(!x.has_avx2() && !x.has_bmi2() && !x.has_avx512f() && !x.has_sha());
    let e = cpu
        .get_extended_processor_and_feature_identifiers()
        .unwrap();
    assert!(e.has_lahf_sahf() && e.has_lzcnt() && e.has_syscall_sysret());
    assert!(e.has_execute_disable() && e.has_rdtscp() && e.has_64bit_mode());
    assert!(!e.has_prefetchw() && !e.has_1gib_pages());
    let state = cpu.get_extended_state_info().unwrap();
    assert!(!state.xcr0_supports_legacy_x87() && state.xsave_area_size_supported_features() == 0);
}

/// The AVX setting adds the x86-64-v3 level and its XSAVE state, x87, SSE and AVX.
#[test]
fn the_avx_setting() {
    let cpu = cpu(true);
    let f = cpu.get_feature_info().unwrap();
    assert!(f.has_avx() && f.has_fma() && f.has_f16c() && f.has_xsave() && f.has_oxsave());
    let x = cpu.get_extended_feature_info().unwrap();
    assert!(x.has_avx2() && x.has_bmi2() && x.has_bmi1() && !x.has_avx512f());
    let state = cpu.get_extended_state_info().unwrap();
    assert!(state.xcr0_supports_legacy_x87() && state.xcr0_supports_sse_128());
    assert!(state.xcr0_supports_avx_256() && !state.xcr0_supports_avx512_opmask());
    // Sixteen 16-byte upper halves after the legacy area and the 64-byte header, and nothing after them.
    let avx = state
        .iter()
        .find(|component| component.is_in_xcr0() && component.size() == 16 * 16)
        .expect("the AVX state");
    let legacy = size_of::<wcontainers_x86::Fxsave>() as u32;
    assert_eq!(avx.offset(), legacy + 64);
    assert_eq!(
        state.xsave_area_size_supported_features(),
        avx.offset() + avx.size()
    );
}

/// Caches and topology of our design: each logical processor a package of its own, sharing no cache, so
/// no count is fixed and any count the kernel reports agrees.
#[test]
fn caches_and_topology() {
    let cpu = cpu(false);
    let caches: Vec<_> = cpu
        .get_cache_parameters()
        .unwrap()
        .map(|c| {
            assert_eq!(c.max_cores_for_cache(), 1);
            assert_eq!(c.coherency_line_size(), LINE_BYTES as usize);
            (
                c.level(),
                c.cache_type(),
                c.associativity() * c.sets() * c.coherency_line_size(),
            )
        })
        .collect();
    let kib = |n: u32| (u64::from(n) * wcontainers_abi::KIB) as usize;
    assert_eq!(
        caches,
        [
            (1, CacheType::Data, kib(L1_KIB)),
            (1, CacheType::Instruction, kib(L1_KIB)),
            (2, CacheType::Unified, kib(L2_KIB)),
            (3, CacheType::Unified, kib(L3_KIB)),
        ]
    );
    let l2 = cpu.get_l2_l3_cache_and_tlb_info().unwrap();
    assert_eq!(
        (
            u32::from(l2.l2cache_size()),
            u32::from(l2.l2cache_line_size())
        ),
        (L2_KIB, LINE_BYTES)
    );
    let levels: Vec<_> = cpu
        .get_extended_topology_info()
        .unwrap()
        .map(|l| (l.level_type(), l.processors(), l.x2apic_id()))
        .collect();
    assert_eq!(
        levels,
        [(TopologyType::SMT, 1, 0), (TopologyType::Core, 1, 0)]
    );
    // An invalid level gives its number back.
    for sub_leaf in [2, 3, 0xff, 0x1_23] {
        assert_eq!(answer(false, 0xb, sub_leaf), [0, 0, sub_leaf & 0xff, 0]);
    }
}

/// A 1 GHz time stamp counter that runs at one rate, and 48-bit addresses.
#[test]
fn time_and_addresses() {
    let cpu = cpu(false);
    let tsc = cpu.get_tsc_info().unwrap();
    assert_eq!(tsc.tsc_frequency(), Some(TIME_STAMP_HZ.into()));
    assert!(
        cpu.get_advanced_power_mgmt_info()
            .unwrap()
            .has_invariant_tsc()
    );
    let sizes = cpu.get_processor_capacity_feature_info().unwrap();
    assert_eq!(
        u32::from(sizes.linear_address_bits()),
        wcontainers_x86::VIRTUAL_ADDRESS_BITS
    );
}

/// Leaves above the highest basic or extended one answer as the highest basic one, as Intel's do.
#[test]
fn leaves_past_the_highest() {
    let top = answer(false, MAX_BASIC, 0);
    for leaf in [MAX_BASIC + 1, EXTENDED - 1, MAX_EXTENDED + 1, u32::MAX] {
        assert_eq!(answer(false, leaf, 0), top, "{leaf:#x}");
    }
    assert_eq!(answer(false, 0, 0)[0], MAX_BASIC);
    assert_eq!(answer(false, EXTENDED, 0)[0], MAX_EXTENDED);
}
