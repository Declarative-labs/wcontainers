//! What XED's CPL field leaves out: instructions it marks as ring 3 that the Intel and AMD manuals say behave otherwise there.

use super::form::Ring3Behavior;

const TIME_STAMP_DISABLE: &str = "Faults at ring 3 when CR4.TSD is set.";
const USER_MODE_INSTRUCTION_PREVENTION: &str = "Faults at ring 3 when CR4.UMIP is set.";
const INPUT_OUTPUT_PRIVILEGE: &str =
    "When CPL is above IOPL the I/O permission bitmap decides, else the port is accessed.";
const INTERRUPT_FLAG_PRIVILEGE: &str = "Faults when CPL is above IOPL.";
const FLAGS_RESTORE_PRIVILEGE: &str = "Changes IF only when CPL is at most IOPL and IOPL only at ring 0, leaving them unchanged otherwise.";
const INTERRUPT_GATE_PRIVILEGE: &str = "Faults when CPL is above the DPL of the IDT gate.";
const DESCRIPTOR_PRIVILEGE: &str = "The result depends on the descriptor's DPL compared with CPL.";
const SEGMENT_LOAD_PRIVILEGE: &str =
    "Checks the selector's RPL and the descriptor's DPL against CPL.";
const FAR_TRANSFER_PRIVILEGE: &str =
    "Checks privilege and may change it through a call gate, switching stacks.";
const SHADOW_STACK_ENABLE: &str = "Enabled by IA32_U_CET at ring 3 and by IA32_S_CET at ring 0.";
const SECURE_VIRTUAL_MACHINE: &str = "Faults when CPL is not 0 (AMD64 manual, volume 3).";

/// Instruction classes that XED marks as ring 3 whose behavior differs between ring 3 and ring 0.
const DEPENDS_ON_PRIVILEGE_CLASSES: [(&str, &str); 53] = [
    (
        "CPUID",
        "Faults at ring 3 when CPUID faulting is on (MSR_MISC_FEATURES_ENABLES bit 0).",
    ),
    ("RDTSC", TIME_STAMP_DISABLE),
    ("RDTSCP", TIME_STAMP_DISABLE),
    ("RDPRU", TIME_STAMP_DISABLE),
    ("RDPMC", "Faults at ring 3 unless CR4.PCE is set."),
    ("SGDT", USER_MODE_INSTRUCTION_PREVENTION),
    ("SIDT", USER_MODE_INSTRUCTION_PREVENTION),
    ("SLDT", USER_MODE_INSTRUCTION_PREVENTION),
    ("STR", USER_MODE_INSTRUCTION_PREVENTION),
    ("SMSW", USER_MODE_INSTRUCTION_PREVENTION),
    ("IN", INPUT_OUTPUT_PRIVILEGE),
    ("OUT", INPUT_OUTPUT_PRIVILEGE),
    ("INSB", INPUT_OUTPUT_PRIVILEGE),
    ("INSW", INPUT_OUTPUT_PRIVILEGE),
    ("INSD", INPUT_OUTPUT_PRIVILEGE),
    ("OUTSB", INPUT_OUTPUT_PRIVILEGE),
    ("OUTSW", INPUT_OUTPUT_PRIVILEGE),
    ("OUTSD", INPUT_OUTPUT_PRIVILEGE),
    ("REP_INSB", INPUT_OUTPUT_PRIVILEGE),
    ("REP_INSW", INPUT_OUTPUT_PRIVILEGE),
    ("REP_INSD", INPUT_OUTPUT_PRIVILEGE),
    ("REP_OUTSB", INPUT_OUTPUT_PRIVILEGE),
    ("REP_OUTSW", INPUT_OUTPUT_PRIVILEGE),
    ("REP_OUTSD", INPUT_OUTPUT_PRIVILEGE),
    ("CLI", INTERRUPT_FLAG_PRIVILEGE),
    ("STI", INTERRUPT_FLAG_PRIVILEGE),
    ("POPF", FLAGS_RESTORE_PRIVILEGE),
    ("POPFQ", FLAGS_RESTORE_PRIVILEGE),
    ("IRET", FLAGS_RESTORE_PRIVILEGE),
    ("IRETD", FLAGS_RESTORE_PRIVILEGE),
    ("IRETQ", FLAGS_RESTORE_PRIVILEGE),
    ("INT", INTERRUPT_GATE_PRIVILEGE),
    ("INT3", INTERRUPT_GATE_PRIVILEGE),
    ("LAR", DESCRIPTOR_PRIVILEGE),
    ("LSL", DESCRIPTOR_PRIVILEGE),
    ("VERR", DESCRIPTOR_PRIVILEGE),
    ("VERW", DESCRIPTOR_PRIVILEGE),
    ("LSS", SEGMENT_LOAD_PRIVILEGE),
    ("LFS", SEGMENT_LOAD_PRIVILEGE),
    ("LGS", SEGMENT_LOAD_PRIVILEGE),
    ("CALL_FAR", FAR_TRANSFER_PRIVILEGE),
    ("JMP_FAR", FAR_TRANSFER_PRIVILEGE),
    ("RET_FAR", FAR_TRANSFER_PRIVILEGE),
    (
        "GETSEC",
        "The leaf in EAX decides: CAPABILITIES runs at every level, the others need ring 0.",
    ),
    ("ENCLU", "Raises #UD below ring 3."),
    ("INCSSPD", SHADOW_STACK_ENABLE),
    ("INCSSPQ", SHADOW_STACK_ENABLE),
    ("RDSSPD", SHADOW_STACK_ENABLE),
    ("RDSSPQ", SHADOW_STACK_ENABLE),
    ("SAVEPREVSSP", SHADOW_STACK_ENABLE),
    ("RSTORSSP", SHADOW_STACK_ENABLE),
    ("WRSSD", SHADOW_STACK_ENABLE),
    ("WRSSQ", SHADOW_STACK_ENABLE),
];

/// The forms of MOV and POP that load a segment register; the other forms of those classes do not.
const DEPENDS_ON_PRIVILEGE_FORMS: [(&str, &str); 4] = [
    ("MOV_SEG_GPR16", SEGMENT_LOAD_PRIVILEGE),
    ("MOV_SEG_MEMw", SEGMENT_LOAD_PRIVILEGE),
    ("POP_FS", SEGMENT_LOAD_PRIVILEGE),
    ("POP_GS", SEGMENT_LOAD_PRIVILEGE),
];

/// Instruction classes that XED marks as ring 3 which fault at ring 3.
const FAULTS_AT_RING3_CLASSES: [(&str, &str); 7] = [
    ("VMRUN", SECURE_VIRTUAL_MACHINE),
    ("VMLOAD", SECURE_VIRTUAL_MACHINE),
    ("VMSAVE", SECURE_VIRTUAL_MACHINE),
    ("STGI", SECURE_VIRTUAL_MACHINE),
    ("CLGI", SECURE_VIRTUAL_MACHINE),
    ("SKINIT", SECURE_VIRTUAL_MACHINE),
    ("ENCLV", "Faults when CPL is not 0 (Intel SDM, volume 3D)."),
];

/// The lowest privilege level XED gives instructions that run at every level.
pub const RING3: u8 = 3;

/// Returns how the instruction behaves at ring 3, and the reason where it is not XED's CPL field alone.
pub fn classify_ring3_behavior(
    iclass: &str,
    iform: &str,
    xed_cpl: u8,
) -> (Ring3Behavior, Option<&'static str>) {
    let find = |table: &[(&str, &'static str)], key: &str| {
        table
            .iter()
            .find(|(name, _)| *name == key)
            .map(|&(_, reason)| reason)
    };
    if xed_cpl != RING3 {
        return (Ring3Behavior::Faults, None);
    }
    if let Some(reason) = find(&FAULTS_AT_RING3_CLASSES, iclass) {
        return (Ring3Behavior::Faults, Some(reason));
    }
    match find(&DEPENDS_ON_PRIVILEGE_CLASSES, iclass).or(find(&DEPENDS_ON_PRIVILEGE_FORMS, iform)) {
        Some(reason) => (Ring3Behavior::DependsOnPrivilegeLevel, Some(reason)),
        None => (Ring3Behavior::Unprivileged, None),
    }
}

/// Every class and form the tables name, so a test can check each is one XED knows.
pub fn list_named_classes_and_forms() -> (Vec<&'static str>, Vec<&'static str>) {
    let classes = DEPENDS_ON_PRIVILEGE_CLASSES
        .iter()
        .chain(&FAULTS_AT_RING3_CLASSES)
        .map(|&(name, _)| name)
        .collect();
    let forms = DEPENDS_ON_PRIVILEGE_FORMS
        .iter()
        .map(|&(name, _)| name)
        .collect();
    (classes, forms)
}
