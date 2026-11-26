//! The XSAVE feature set's state as a program at ring 3 sees it, and the forms that read it: XGETBV, XSAVE, XSAVEC,
//! XSAVEOPT and XRSTOR, as the [Intel SDM](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html)
//! volume 1, chapter 13, and volume 2's pages give them; and UMWAIT and TPAUSE, whose CF the time decides.
//!
//! XCR0 names the components the operating system enables. XINUSE is the chip's init tracking: a clear bit says its
//! component holds its initial configuration, a set bit says nothing, and an instruction that writes a component
//! sets its bit. The modified optimization's tracking is the area the last XRSTOR at ring 3 restored from and which
//! components in use nothing wrote since; a tracking where every component in use was written since skips nothing,
//! so it is the same as none.

use alloc::vec::Vec;

use iced_x86::{Code, Instruction as Decoded, InstructionInfoFactory, OpAccess, Register};

use crate::behavior::StoreAcrossFault;
use crate::{Instruction, PreciseInstruction, Processor, X87_INITIAL};

/// XCR0's component bits: x87, SSE, AVX, opmask, ZMM_Hi256, Hi16_ZMM and PKRU.
pub const X87_COMPONENT: u64 = 1 << 0;
pub const SSE_COMPONENT: u64 = 1 << 1;
pub const AVX_COMPONENT: u64 = 1 << 2;
pub const OPMASK_COMPONENT: u64 = 1 << 5;
pub const ZMM_HIGH_256_COMPONENT: u64 = 1 << 6;
pub const HIGH_16_ZMM_COMPONENT: u64 = 1 << 7;
pub const PKRU_COMPONENT: u64 = 1 << 9;
/// AMX's TILECFG and TILEDATA.
pub const TILE_COMPONENTS: u64 = 1 << 17 | 1 << 18;

/// XCR0 as Linux sets it on a chip with AVX-512 and protection keys and without AMX: x87, SSE, AVX, the three AVX-512
/// components and PKRU. On this chip Linux's XCR0 also enables AMX's two components (0x602E7), which KVM gives a
/// guest only once its host process asks for them with `arch_prctl(ARCH_REQ_XCOMP_GUEST_PERM)`, a call no crate this
/// workspace allows makes safely; until it does, every state holds this value.
pub const LINUX_XCR0: u64 = X87_COMPONENT
    | SSE_COMPONENT
    | AVX_COMPONENT
    | OPMASK_COMPONENT
    | ZMM_HIGH_256_COMPONENT
    | HIGH_16_ZMM_COMPONENT
    | PKRU_COMPONENT;

/// The components every generated state holds in use unless its form says otherwise: x87, SSE, AVX and PKRU.
pub const LEGACY_AND_KEY_COMPONENTS: u64 = X87_COMPONENT | SSE_COMPONENT | AVX_COMPONENT | PKRU_COMPONENT;

/// MXCSR's value at reset, which XSAVEC and XGETBV compare it with.
pub const MXCSR_RESET: u32 = 0x1F80;

/// What the modified optimization knows: the linear address of the area the last XRSTOR at ring 3 restored from, in
/// the standard form, and the components in use that nothing wrote since, of which there is at least one.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct XsaveTracking {
    pub area: u64,
    pub unmodified: u64,
}

/// Where XSAVE's standard format keeps a component above the legacy region and its header, and its bytes, as this
/// chip's CPUID leaf 0xD gives them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ExtendedComponent {
    pub bit: u64,
    pub offset: u64,
    pub bytes: u64,
}

/// AVX, opmask, ZMM_Hi256, Hi16_ZMM and PKRU, in order; none of them asks for 64-byte alignment in the compacted
/// format, and each size but PKRU's is a multiple of 64.
pub const EXTENDED_COMPONENTS: [ExtendedComponent; 5] = [
    ExtendedComponent { bit: AVX_COMPONENT, offset: 576, bytes: 256 },
    ExtendedComponent { bit: OPMASK_COMPONENT, offset: 1088, bytes: 64 },
    ExtendedComponent { bit: ZMM_HIGH_256_COMPONENT, offset: 1152, bytes: 512 },
    ExtendedComponent { bit: HIGH_16_ZMM_COMPONENT, offset: 1664, bytes: 1024 },
    ExtendedComponent { bit: PKRU_COMPONENT, offset: 2688, bytes: 8 },
];

/// The legacy region's bytes, which hold x87's and SSE's components.
pub const LEGACY_REGION_BYTES: u64 = 512;
/// The header's offset and bytes.
pub const HEADER_OFFSET: u64 = 512;
pub const HEADER_BYTES: u64 = 64;
/// The standard format's bytes under [`LINUX_XCR0`]: up to the end of PKRU's component.
pub const STANDARD_AREA_BYTES: u64 = 2696;
/// The alignment every area of the family needs.
pub const AREA_ALIGNMENT: u64 = 64;

/// The offset of each extended component of the request: the standard format's, or the compacted format's, where each
/// follows the last before it.
pub fn list_component_offsets(requested: u64, standard: bool) -> Vec<(ExtendedComponent, u64)> {
    let mut next = HEADER_OFFSET + HEADER_BYTES;
    EXTENDED_COMPONENTS
        .into_iter()
        .filter(|component| requested & component.bit != 0)
        .map(|component| {
            let offset = if standard { component.offset } else { next };
            next = offset + component.bytes;
            (component, offset)
        })
        .collect()
}

/// The bytes a form's access spans from the area's start: to the header's end, or to the end of the last component
/// requested in the format's layout.
pub fn to_extent(requested: u64, standard: bool) -> u64 {
    list_component_offsets(requested, standard)
        .last()
        .map_or(HEADER_OFFSET + HEADER_BYTES, |(component, offset)| offset + component.bytes)
}

/// Whether the component holds its initial configuration in the processor.
pub fn is_initial(processor: &Processor, component: u64) -> bool {
    match component {
        X87_COMPONENT => {
            let x87 = &processor.x87;
            x87.control == X87_INITIAL.control
                && x87.status == 0
                && x87.tags == 0
                && x87.instruction_pointer == 0
                && x87.data_pointer == 0
                && x87.registers.iter().all(|register| *register == 0)
        }
        SSE_COMPONENT => processor.vector_registers.iter().all(|value| *value == 0),
        AVX_COMPONENT => processor.upper_vector_halves.iter().all(|value| *value == 0),
        OPMASK_COMPONENT => processor.opmask_registers.iter().all(|value| *value == 0),
        ZMM_HIGH_256_COMPONENT => processor.zmm_upper_halves.iter().flatten().all(|value| *value == 0),
        HIGH_16_ZMM_COMPONENT => processor.high_vector_registers.iter().flatten().all(|value| *value == 0),
        PKRU_COMPONENT => processor.pkru == 0,
        _ => true,
    }
}

/// The components of XCR0 that do not hold their initial configuration.
pub fn list_components_not_initial(processor: &Processor) -> u64 {
    list_components(processor.xcr0)
        .filter(|component| !is_initial(processor, *component))
        .fold(0, |all, component| all | component)
}

/// Each component bit of the bitmap that this architecture models, lowest first.
pub fn list_components(bitmap: u64) -> impl Iterator<Item = u64> {
    [X87_COMPONENT, SSE_COMPONENT, AVX_COMPONENT, OPMASK_COMPONENT, ZMM_HIGH_256_COMPONENT, HIGH_16_ZMM_COMPONENT, PKRU_COMPONENT]
        .into_iter()
        .filter(move |component| bitmap & component != 0)
}

/// Sets XINUSE's bit of every component that does not hold its initial configuration, as a state must have it.
pub fn mark_components_in_use(processor: &mut Processor) {
    processor.xinuse |= list_components_not_initial(processor);
}

/// Whether XINUSE and the tracking say what the processor can hold: no bit outside XCR0, no component out of its
/// initial configuration marked unused, and a tracking whose unmodified components are in use and not none.
pub fn is_consistent(processor: &Processor) -> bool {
    let tracking_holds = processor
        .xsave_tracking
        .is_none_or(|tracking| tracking.unmodified != 0 && tracking.unmodified & !processor.xinuse == 0);
    processor.xinuse & !processor.xcr0 == 0
        && list_components_not_initial(processor) & !processor.xinuse == 0
        && tracking_holds
}

/// The components whose registers differ between the two processors.
pub fn list_changed_components(before: &Processor, after: &Processor) -> u64 {
    let pairs = [
        (X87_COMPONENT, before.x87 != after.x87),
        (SSE_COMPONENT, before.vector_registers != after.vector_registers),
        (AVX_COMPONENT, before.upper_vector_halves != after.upper_vector_halves),
        (OPMASK_COMPONENT, before.opmask_registers != after.opmask_registers),
        (ZMM_HIGH_256_COMPONENT, before.zmm_upper_halves != after.zmm_upper_halves),
        (HIGH_16_ZMM_COMPONENT, before.high_vector_registers != after.high_vector_registers),
        (PKRU_COMPONENT, before.pkru != after.pkru),
    ];
    pairs.into_iter().filter(|(_, changed)| *changed).fold(0, |all, (component, _)| all | component)
}

/// The components an instruction names a register of to write, as iced-x86 lists them: an XMM register of 0 to 15
/// is SSE's, a YMM or ZMM one SSE's and AVX's and, from VEX and EVEX, ZMM_Hi256's, which they zero; a vector
/// register above 15 Hi16_ZMM's; an opmask register opmask's; an x87 or MMX register x87's.
pub fn list_written_components(decoded: &Decoded) -> u64 {
    let mut factory = InstructionInfoFactory::new();
    let widened = matches!(decoded.encoding(), iced_x86::EncodingKind::VEX | iced_x86::EncodingKind::EVEX);
    let mut written = 0;
    for used in factory.info(decoded).used_registers() {
        let access = used.access();
        if !matches!(access, OpAccess::Write | OpAccess::CondWrite | OpAccess::ReadWrite | OpAccess::ReadCondWrite) {
            continue;
        }
        let register: Register = used.register();
        written |= if register.is_k() {
            OPMASK_COMPONENT
        } else if register.is_st() || register.is_mm() {
            X87_COMPONENT
        } else if (register.is_xmm() || register.is_ymm() || register.is_zmm()) && register.number() >= 16 {
            HIGH_16_ZMM_COMPONENT
        } else if register.is_xmm() && !widened {
            SSE_COMPONENT
        } else if register.is_xmm() || register.is_ymm() || register.is_zmm() {
            SSE_COMPONENT | AVX_COMPONENT | ZMM_HIGH_256_COMPONENT
        } else {
            0
        };
    }
    written
}

/// Updates XINUSE and the tracking after an instruction of another family: each component it changed, or that it
/// names a register of to write and completes, is in use and modified, but those VZEROUPPER and VZEROALL zero where
/// the behavior tracks them as initial. The forms of this family set them themselves.
pub(crate) fn track_components(behavior: PreciseInstruction, before: &Processor, decoded: Option<&Decoded>, after: &mut Processor) {
    if matches!(behavior, PreciseInstruction::State { .. }) {
        return;
    }
    let mut touched = list_changed_components(before, after);
    if let Some(decoded) = decoded
        && after.exception.is_none()
        && (after.xinuse & !touched != 0 || after.xsave_tracking.is_some())
        && !crate::planted::is_active(crate::planted::Bug::InUseIgnoresSameValueWrites)
    {
        touched |= list_written_components(decoded);
    }
    touched &= after.xcr0;
    let zeroed = if after.exception.is_none() { list_zeroed_components(behavior) } else { 0 };
    after.xinuse = (after.xinuse | touched) & !zeroed;
    after.xsave_tracking = after.xsave_tracking.and_then(|tracking| to_tracking(tracking.area, tracking.unmodified & !touched, after.xinuse));
}

/// The tracking of the area with these components unmodified, those not in use dropped; none where none is left.
pub fn to_tracking(area: u64, unmodified: u64, xinuse: u64) -> Option<XsaveTracking> {
    let unmodified = unmodified & xinuse;
    (unmodified != 0).then_some(XsaveTracking { area, unmodified })
}

/// What XINUSE says of the components VZEROUPPER and VZEROALL put in their initial configuration, which the manual
/// leaves to the chip: tracked as initial, or still in use.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ZeroedTracking {
    TrackedInitial,
    TrackedInUse,
}

/// The tracking a form that zeroes no component takes first.
pub const ZEROED_TRACKINGS: [ZeroedTracking; 2] = [ZeroedTracking::TrackedInUse, ZeroedTracking::TrackedInitial];

/// The components VZEROUPPER (AVX's and ZMM_Hi256) or VZEROALL (SSE's too) put in their initial configuration, for
/// a behavior that tracks them as initial.
fn list_zeroed_components(behavior: PreciseInstruction) -> u64 {
    let PreciseInstruction::Avx { instruction, behavior } = behavior else {
        return 0;
    };
    if behavior.zeroed != ZeroedTracking::TrackedInitial {
        return 0;
    }
    match instruction {
        Instruction::Vzeroupper if crate::planted::is_active(crate::planted::Bug::ZeroUpperTracksSseInitial) => {
            SSE_COMPONENT | AVX_COMPONENT | ZMM_HIGH_256_COMPONENT
        }
        Instruction::Vzeroupper => AVX_COMPONENT | ZMM_HIGH_256_COMPONENT,
        Instruction::Vzeroall => SSE_COMPONENT | AVX_COMPONENT | ZMM_HIGH_256_COMPONENT,
        _ => 0,
    }
}

/// What a form of this family does.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StateOperation {
    /// XGETBV: EDX:EAX from the XCR ECX names, XCR0 or (ECX 1) XCR0 AND XINUSE.
    ReadControl,
    /// XSAVE, XSAVEC or XSAVEOPT; `wide` for the forms under REX.W, which save FIP and FDP whole.
    Save { format: SaveFormat, wide: bool },
    /// XRSTOR, in the standard or the compacted form its header chooses.
    Restore { wide: bool },
    /// UMWAIT or TPAUSE: a wait whose end the time decides.
    Wait,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SaveFormat {
    /// XSAVE: every requested component, in the standard format.
    Standard,
    /// XSAVEOPT: the standard format with the init and modified optimizations.
    Optimized,
    /// XSAVEC: the compacted format with the init optimization.
    Compacted,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StateForm {
    pub instruction: Instruction,
    pub operation: StateOperation,
    pub code: Code,
}

/// The row of a form of this family, or `None` for any other form.
pub fn find_state_form(instruction: Instruction) -> Option<&'static StateForm> {
    STATE_FORMS.iter().find(|form| form.instruction == instruction)
}

const fn state(instruction: Instruction, operation: StateOperation, code: Code) -> StateForm {
    StateForm { instruction, operation, code }
}

const fn save(format: SaveFormat, wide: bool) -> StateOperation {
    StateOperation::Save { format, wide }
}

/// Every form of this family modeled.
pub static STATE_FORMS: &[StateForm] = &[
    state(Instruction::Xgetbv, StateOperation::ReadControl, Code::Xgetbv),
    state(Instruction::XsaveMemmxsave, save(SaveFormat::Standard, false), Code::Xsave_mem),
    state(Instruction::Xsave64Memmxsave, save(SaveFormat::Standard, true), Code::Xsave64_mem),
    state(Instruction::XsaveoptMemmxsave, save(SaveFormat::Optimized, false), Code::Xsaveopt_mem),
    state(Instruction::Xsaveopt64Memmxsave, save(SaveFormat::Optimized, true), Code::Xsaveopt64_mem),
    state(Instruction::XsavecMemmxsave, save(SaveFormat::Compacted, false), Code::Xsavec_mem),
    state(Instruction::Xsavec64Memmxsave, save(SaveFormat::Compacted, true), Code::Xsavec64_mem),
    state(Instruction::XrstorMemmxsave, StateOperation::Restore { wide: false }, Code::Xrstor_mem),
    state(Instruction::Xrstor64Memmxsave, StateOperation::Restore { wide: true }, Code::Xrstor64_mem),
    state(Instruction::UmwaitGpr32, StateOperation::Wait, Code::Umwait_r32),
    state(Instruction::TpauseGpr32u32, StateOperation::Wait, Code::Tpause_r32),
];

/// What the manual leaves to the chip for an area not aligned to 64 while AC is set (CR0.AM is at ring 3): #GP, or
/// #AC for an address not aligned to the width the chip checks and #GP for one that is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MisalignedArea {
    GeneralProtection,
    AlignmentCheckBelow { width: u64 },
}

/// What XRSTOR leaves in XINUSE for a requested component it puts in its initial configuration: tracked as initial,
/// in use, or as initial but PKRU, which a chip may not track; PKRU left in use so is written, and so modified.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RestoredInitial {
    TrackedInitial,
    TrackedInUse,
    TrackedInitialButPkru,
}

/// What UMWAIT and TPAUSE do at ring 3: wait, or raise #GP(0) because CR4.TSD refuses the time-stamp counter to it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WaitControl {
    Waits,
    FaultsUnderTimeStampDisable,
}

/// A form's behavior where the manuals leave a choice.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StateBehavior {
    /// XGETBV, which leaves no choice.
    ReadControl,
    /// An XSAVE form: what a misaligned area raises under AC and what a store that faults part-way writes.
    Save { misaligned: MisalignedArea, across_fault: StoreAcrossFault },
    Restore { misaligned: MisalignedArea, restored_initial: RestoredInitial },
    Wait(WaitControl),
}

const MISALIGNED: [MisalignedArea; 6] = [
    MisalignedArea::GeneralProtection,
    MisalignedArea::AlignmentCheckBelow { width: 2 },
    MisalignedArea::AlignmentCheckBelow { width: 4 },
    MisalignedArea::AlignmentCheckBelow { width: 8 },
    MisalignedArea::AlignmentCheckBelow { width: 16 },
    MisalignedArea::AlignmentCheckBelow { width: 64 },
];
const ACROSS_FAULT: [StoreAcrossFault; 2] = [StoreAcrossFault::WritesNothing, StoreAcrossFault::WritesBytesBelowFault];
const RESTORED_INITIAL: [RestoredInitial; 3] =
    [RestoredInitial::TrackedInitial, RestoredInitial::TrackedInUse, RestoredInitial::TrackedInitialButPkru];

/// Every combination of the choices the form leaves open, the documented one first.
pub(crate) fn list_state_behaviors(instruction: Instruction) -> Vec<PreciseInstruction> {
    let Some(form) = find_state_form(instruction) else {
        return Vec::new();
    };
    let behaviors: Vec<StateBehavior> = match form.operation {
        StateOperation::ReadControl => [StateBehavior::ReadControl].to_vec(),
        StateOperation::Save { .. } => MISALIGNED
            .iter()
            .flat_map(|&misaligned| ACROSS_FAULT.map(|across_fault| StateBehavior::Save { misaligned, across_fault }))
            .collect(),
        StateOperation::Restore { .. } => MISALIGNED
            .iter()
            .flat_map(|&misaligned| {
                RESTORED_INITIAL.map(|restored_initial| StateBehavior::Restore { misaligned, restored_initial })
            })
            .collect(),
        StateOperation::Wait => [WaitControl::Waits, WaitControl::FaultsUnderTimeStampDisable].map(StateBehavior::Wait).to_vec(),
    };
    behaviors.into_iter().map(|behavior| PreciseInstruction::State { instruction, behavior }).collect()
}

/// Whether the behavior is UMWAIT's or TPAUSE's waiting one, whose CF the time decides.
pub(crate) fn is_wait(behavior: PreciseInstruction) -> bool {
    matches!(behavior, PreciseInstruction::State { behavior: StateBehavior::Wait(WaitControl::Waits), .. })
}
