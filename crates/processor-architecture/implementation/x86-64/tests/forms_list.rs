//! Checks the instruction list against its data file, and the data file against itself.

#[path = "../forms/generation/mod.rs"]
#[allow(dead_code, reason = "the generator and each test use different parts")]
mod generation;

use std::collections::BTreeSet;
use std::fs;

use generation::form::{
    Access, FlagCondition, InstructionForm, LONG_MODE, OperandKind, Ring3Behavior,
    derive_variant_name, parse_data_file,
};
use generation::privilege::{RING3, list_named_classes_and_forms};
use generation::render::render_module;
use generation::{DATA_FILE, MODULE_FILE, locate_in_crate};
use wcontainers_processor_architecture_x86_64::{INSTRUCTIONS, XED_VERSION, list_forms as list_instructions};

/// The modes an encoding can be valid in, by default address size.
const MODES: [u8; 3] = [16, 32, LONG_MODE];
/// The flags XED names: the status and control flags of RFLAGS, then the x87 condition codes C0 to C3.
const FLAGS: [&str; 21] = [
    "of", "sf", "zf", "af", "pf", "cf", "df", "if", "tf", "iopl", "nt", "rf", "vm", "ac", "vif",
    "vip", "id", "fc0", "fc1", "fc2", "fc3",
];
const CPUID_REGISTERS: [&str; 4] = ["EAX", "EBX", "ECX", "EDX"];
const CPUID_REGISTER_BITS: u8 = u32::BITS as u8;
/// ModRM.reg and ModRM.rm are three bits each.
const MODRM_FIELD_VALUES: u8 = 1 << 3;
const NO_PRIVILEGE: u8 = 0;

fn read_forms() -> Vec<InstructionForm> {
    let text = fs::read_to_string(locate_in_crate(DATA_FILE)).expect("the data file is readable");
    parse_data_file(&text).expect("every line of the data file is a record")
}

#[test]
fn module_is_rendered_from_the_data_file() {
    let module = fs::read_to_string(locate_in_crate(MODULE_FILE))
        .expect("the instruction module is readable");
    assert!(
        module == render_module(&read_forms(), XED_VERSION),
        "{MODULE_FILE} is not what the data file renders: run the generator"
    );
}

#[test]
fn list_follows_the_data_file() {
    let listed: Vec<String> = list_instructions()
        .map(|instruction| format!("{instruction:?}"))
        .collect();
    let recorded: Vec<String> = read_forms().into_iter().map(|form| form.name).collect();
    assert_eq!(listed, recorded);
}

#[test]
fn instructions_are_distinct_and_in_declaration_order() {
    assert!(INSTRUCTIONS.windows(2).all(|pair| pair[0] < pair[1]));
}

#[test]
fn names_follow_the_iforms() {
    let forms = read_forms();
    assert!(
        forms.windows(2).all(|pair| pair[0].iform < pair[1].iform),
        "the iforms are distinct and sorted"
    );
    let names: BTreeSet<&str> = forms.iter().map(|form| form.name.as_str()).collect();
    assert_eq!(names.len(), forms.len(), "no two iforms share a name");
    for form in &forms {
        assert_eq!(form.name, derive_variant_name(&form.iform));
        let mut characters = form.name.chars();
        assert!(
            characters
                .next()
                .is_some_and(|first| first.is_ascii_uppercase())
                && characters.all(|character| character.is_ascii_alphanumeric()),
            "{} is an UpperCamelCase identifier",
            form.name
        );
    }
}

#[test]
fn every_encoding_is_valid_in_64_bit_mode() {
    for form in read_forms() {
        assert!(!form.encodings.is_empty(), "{} has an encoding", form.iform);
        for encoding in &form.encodings {
            assert!(
                encoding.modes.contains(&LONG_MODE),
                "{}: {}",
                form.iform,
                encoding.pattern
            );
            assert!(encoding.modes.iter().all(|mode| MODES.contains(mode)));
            if let Some(modrm) = &encoding.modrm {
                assert!(
                    modrm
                        .reg
                        .iter()
                        .chain(&modrm.rm)
                        .all(|value| *value < MODRM_FIELD_VALUES)
                );
            }
        }
    }
}

#[test]
fn privilege_follows_xed_or_names_a_reason() {
    let forms = read_forms();
    for form in &forms {
        let consistent = matches!(
            (
                form.xed_cpl,
                form.ring3_behavior,
                &form.ring3_behavior_reason
            ),
            (NO_PRIVILEGE, Ring3Behavior::Faults, None)
                | (RING3, Ring3Behavior::Unprivileged, None)
                | (
                    RING3,
                    Ring3Behavior::Faults | Ring3Behavior::DependsOnPrivilegeLevel,
                    Some(_)
                )
        );
        assert!(
            consistent,
            "{}: XED's CPL {} and {:?}",
            form.iform, form.xed_cpl, form.ring3_behavior
        );
    }
    let (classes, iforms) = list_named_classes_and_forms();
    for class in classes {
        assert!(
            forms.iter().any(|form| form.iclass == class),
            "{class} is a class of the list"
        );
    }
    for iform in iforms {
        assert!(
            forms.iter().any(|form| form.iform == iform),
            "{iform} is a form of the list"
        );
    }
}

#[test]
fn operands_and_flags_are_consistent() {
    for form in read_forms() {
        for encoding in &form.encodings {
            let names: BTreeSet<&str> = encoding
                .operands
                .iter()
                .map(|operand| operand.name.as_str())
                .collect();
            assert_eq!(
                names.len(),
                encoding.operands.len(),
                "{}: operand names are distinct",
                form.iform
            );
            for operand in &encoding.operands {
                assert!(
                    operand.kind != OperandKind::Register || operand.register.is_some(),
                    "{}: register operand {} names its register",
                    form.iform,
                    operand.name
                );
                assert!(
                    operand.read != Access::Never || operand.written != Access::Never,
                    "{}: operand {} is read or written",
                    form.iform,
                    operand.name
                );
            }
            for effect in &encoding.flags {
                let written: BTreeSet<&String> = effect.written.iter().collect();
                assert!(
                    effect
                        .undefined
                        .iter()
                        .chain(&effect.cleared)
                        .chain(&effect.set)
                        .all(|flag| written.contains(flag)),
                    "{}: undefined, cleared and set flags are written",
                    form.iform
                );
                assert!(effect.condition != FlagCondition::ReadOnly || written.is_empty());
                assert!(
                    effect
                        .read
                        .iter()
                        .chain(&effect.written)
                        .all(|flag| FLAGS.contains(&flag.as_str())),
                    "{}: every flag is one XED names",
                    form.iform
                );
            }
        }
    }
}

#[test]
fn cpuid_requirements_are_well_formed() {
    for form in read_forms() {
        assert!(!form.isa_set.is_empty());
        for alternative in &form.cpuid {
            assert!(
                !alternative.requirements.is_empty(),
                "{}: {}",
                form.iform,
                alternative.group
            );
            for requirement in &alternative.requirements {
                assert!(CPUID_REGISTERS.contains(&requirement.register.as_str()));
                assert!(requirement.lowest_bit <= requirement.highest_bit);
                assert!(requirement.highest_bit < CPUID_REGISTER_BITS);
            }
        }
    }
}
