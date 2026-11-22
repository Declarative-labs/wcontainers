//! `cpuid`'s designed answers (plan 5.10), read back through raw-cpuid, a parser that knows nothing of ours.

use raw_cpuid::{CacheType, CpuId, CpuIdResult, TopologyType};
use wcontainers_kernel::x86_64_features::{
    Cpuid01Ecx, Cpuid07Ebx, Cpuid07Edx, Cpuid80000001Ecx, ProcessorSignature, X86_64Features,
};
use wcontainers_x86::cpuid::{
    BRAND, EDX, EXTENDED, EXTENDED_FEATURES_LEAF, L1_KIB, L2_KIB, L3_KIB, LINE_BYTES, MAX_BASIC,
    MAX_EXTENDED, TIME_STAMP_HZ, VENDOR, answer,
};
use wcontainers_x86::offer::{REFERENCE_FAMILY, REFERENCE_MODEL, REFERENCE_STEPPING};
use wcontainers_x86::{FULL_SPEED, Offer, Refused, V3};

fn cpu(features: X86_64Features) -> CpuId<impl raw_cpuid::CpuIdReader> {
    let offer = Offer::new(features).expect("a list of built features");
    CpuId::with_cpuid_reader(move |leaf, sub_leaf| {
        let [eax, ebx, ecx, edx] = answer(offer, leaf, sub_leaf);
        CpuIdResult { eax, ebx, ecx, edx }
    })
}

/// The full-speed list: x86-64-v2 and the rest of plan 5.10's list, nothing of x86-64-v3 or beyond.
#[test]
fn the_full_speed_list() {
    let cpu = cpu(FULL_SPEED);
    assert_eq!(cpu.get_vendor_info().unwrap().as_str().as_bytes(), VENDOR);
    assert_eq!(cpu.get_processor_brand_string().unwrap().as_str(), BRAND);
    let f = cpu.get_feature_info().unwrap();
    assert_eq!(
        (
            u32::from(f.family_id()),
            u32::from(f.model_id()),
            u32::from(f.stepping_id())
        ),
        (
            u32::from(REFERENCE_FAMILY),
            u32::from(REFERENCE_MODEL),
            u32::from(REFERENCE_STEPPING)
        )
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

/// The x86-64-v3 list adds its level and its XSAVE state, x87, SSE and AVX.
#[test]
fn the_v3_list() {
    let cpu = cpu(V3);
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
    let cpu = cpu(FULL_SPEED);
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
        assert_eq!(
            answer(Offer::default(), 0xb, sub_leaf),
            [0, 0, sub_leaf & 0xff, 0]
        );
    }
}

/// A 1 GHz time stamp counter that runs at one rate, and 48-bit addresses.
#[test]
fn time_and_addresses() {
    let cpu = cpu(FULL_SPEED);
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
    let offer = Offer::default();
    let top = answer(offer, MAX_BASIC, 0);
    for leaf in [MAX_BASIC + 1, EXTENDED - 1, MAX_EXTENDED + 1, u32::MAX] {
        assert_eq!(answer(offer, leaf, 0), top, "{leaf:#x}");
    }
    assert_eq!(answer(offer, 0, 0)[0], MAX_BASIC);
    assert_eq!(answer(offer, EXTENDED, 0)[0], MAX_EXTENDED);
}

/// Each process's list is what its program reads: a list narrower than full speed takes features away, one
/// of another processor's identity changes leaf 1's signature, and the hints are answered as told.
#[test]
fn each_list_is_what_its_program_reads() {
    let narrow = X86_64Features {
        cpuid_01_ecx: FULL_SPEED.cpuid_01_ecx - Cpuid01Ecx::SSE4_2 - Cpuid01Ecx::POPCNT,
        cpuid_07_ebx: Cpuid07Ebx::empty(),
        cpuid_80000001_ecx: Cpuid80000001Ecx::empty(),
        ..FULL_SPEED
    };
    let c = cpu(narrow);
    let f = c.get_feature_info().unwrap();
    assert!(f.has_sse41() && !f.has_sse42() && !f.has_popcnt());
    let x = c.get_extended_feature_info().unwrap();
    assert!(!x.has_bmi1() && !x.has_adx() && !x.has_fsgsbase());
    let e = c.get_extended_processor_and_feature_identifiers().unwrap();
    assert!(!e.has_lahf_sahf() && !e.has_lzcnt() && e.has_rdtscp());

    // A Sapphire Rapids signature: family 6, model 0x8F, stepping 8; and a family past 0xF.
    for (family, model, stepping) in [(6, 0x8f, 8), (0x19, 0x21, 2)] {
        let signature = ProcessorSignature::new(family, model, stepping).unwrap();
        let c = cpu(X86_64Features {
            signature,
            ..FULL_SPEED
        });
        let f = c.get_feature_info().unwrap();
        assert_eq!(
            (f.family_id(), f.model_id(), f.stepping_id()),
            (family as u8, model, stepping)
        );
    }

    let hinted = X86_64Features {
        cpuid_07_ebx: FULL_SPEED.cpuid_07_ebx | Cpuid07Ebx::ENH_REP_MOVSB_STOSB,
        cpuid_07_edx: Cpuid07Edx::FAST_SHORT_REP_MOVSB,
        ..FULL_SPEED
    };
    let x = cpu(hinted).get_extended_feature_info().unwrap();
    assert!(x.has_rep_movsb_stosb());
    let edx = answer(Offer::new(hinted).unwrap(), EXTENDED_FEATURES_LEAF, 0)[EDX];
    assert_eq!(edx, Cpuid07Edx::FAST_SHORT_REP_MOVSB.bits());
}

/// A list with a feature that has no meanings here is refused, by that feature's name.
#[test]
fn a_list_with_an_unbuilt_feature_is_refused() {
    let with = |ecx| X86_64Features {
        cpuid_01_ecx: FULL_SPEED.cpuid_01_ecx | ecx,
        ..FULL_SPEED
    };
    assert_eq!(
        Offer::new(with(Cpuid01Ecx::AES)),
        Err(Refused::NotBuilt("AES"))
    );
    assert_eq!(
        Offer::new(with(Cpuid01Ecx::RDRAND)),
        Err(Refused::NotBuilt("RDRAND"))
    );
    let avx512 = X86_64Features {
        cpuid_07_ebx: V3.cpuid_07_ebx | Cpuid07Ebx::AVX512F,
        ..V3
    };
    assert_eq!(Offer::new(avx512), Err(Refused::NotBuilt("AVX512F")));
    assert!(Offer::new(V3).is_ok());
    assert_eq!(Offer::default().features(), FULL_SPEED);
}

/// A list no processor could have is refused: AVX without XSAVE, and AVX2, FMA or F16C without AVX.
#[test]
fn a_list_without_a_prerequisite_is_refused() {
    let refused = |feature, prerequisite| {
        Err(Refused::MissingPrerequisite {
            feature,
            prerequisite,
        })
    };
    let without_avx = |ecx: Cpuid01Ecx, ebx: Cpuid07Ebx| X86_64Features {
        cpuid_01_ecx: (V3.cpuid_01_ecx - Cpuid01Ecx::AVX - Cpuid01Ecx::FMA - Cpuid01Ecx::F16C)
            | ecx,
        cpuid_07_ebx: (V3.cpuid_07_ebx - Cpuid07Ebx::AVX2) | ebx,
        ..V3
    };
    assert!(Offer::new(without_avx(Cpuid01Ecx::empty(), Cpuid07Ebx::empty())).is_ok());
    assert_eq!(
        Offer::new(without_avx(Cpuid01Ecx::empty(), Cpuid07Ebx::AVX2)),
        refused("AVX2", "AVX")
    );
    assert_eq!(
        Offer::new(without_avx(Cpuid01Ecx::FMA, Cpuid07Ebx::empty())),
        refused("FMA", "AVX")
    );
    assert_eq!(
        Offer::new(without_avx(Cpuid01Ecx::F16C, Cpuid07Ebx::empty())),
        refused("F16C", "AVX")
    );
    let avx_without_xsave = X86_64Features {
        cpuid_01_ecx: V3.cpuid_01_ecx - Cpuid01Ecx::XSAVE,
        ..V3
    };
    assert_eq!(Offer::new(avx_without_xsave), refused("AVX", "XSAVE"));
}

/// The cache key tells apart lists that differ in one feature or in the signature.
#[test]
fn the_key_tells_lists_apart() {
    let v2 = Offer::default().key();
    let v3 = Offer::new(V3).unwrap().key();
    let other = Offer::new(X86_64Features {
        signature: ProcessorSignature::new(6, 0x8f, 8).unwrap(),
        ..FULL_SPEED
    })
    .unwrap()
    .key();
    let fewer = Offer::new(X86_64Features {
        cpuid_01_ecx: FULL_SPEED.cpuid_01_ecx - Cpuid01Ecx::POPCNT,
        ..FULL_SPEED
    })
    .unwrap()
    .key();
    let keys = [v2, v3, other, fewer];
    for (i, a) in keys.iter().enumerate() {
        for b in &keys[i + 1..] {
            assert_ne!(a, b);
        }
    }
}
