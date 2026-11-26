//! The instruction module, written from the data file's records.

use std::collections::BTreeMap;

use super::form::{CpuidRequirement, InstructionForm};
use super::sample::encode_sample;

/// Returns the source of the instruction module, formatted as rustfmt leaves it.
pub fn render_module(forms: &[InstructionForm], xed_version: &str) -> String {
    let mut module = format!(
        "//! Every x86 instruction form that executes in 64-bit mode, generated from Intel XED by the `generate_forms` example.\n\
         \n\
         /// The XED release the forms come from.\n\
         pub const XED_VERSION: &str = \"{xed_version}\";\n\
         \n\
         /// An x86 [instruction form](https://intelxed.github.io/ref-manual/group__IFORM.html) that executes in 64-bit mode.\n\
         #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]\n\
         pub enum Instruction {{\n"
    );
    for form in forms {
        module.push_str(&format!("    /// `{}`.\n    {},\n", form.iform, form.name));
    }
    module.push_str(&format!(
        "}}\n\
         \n\
         /// Every instruction form, in the order of XED's names.\n\
         pub static INSTRUCTIONS: [Instruction; {}] = [\n",
        forms.len()
    ));
    for form in forms {
        module.push_str(&format!("    Instruction::{},\n", form.name));
    }
    module.push_str(
        "];\n\
         \n\
         /// Returns every instruction form.\n\
         pub fn list_instructions() -> impl Iterator<Item = Instruction> {\n    \
         INSTRUCTIONS.iter().copied()\n\
         }\n",
    );
    render_cpuid_requirements(forms, &mut module);
    render_samples(forms, &mut module);
    module
}

/// Appends the CPUID requirement type, one constant per requirement XED names, and each form's alternatives.
fn render_cpuid_requirements(forms: &[InstructionForm], module: &mut String) {
    module.push_str(
        "\n\
         /// A register of a CPUID result.\n\
         #[derive(Clone, Copy, PartialEq, Eq, Debug)]\n\
         pub enum CpuidRegister {\n    Eax,\n    Ebx,\n    Ecx,\n    Edx,\n}\n\
         \n\
         /// A field of a CPUID result that must hold a value for a form to execute, under XED's name for it.\n\
         #[derive(Clone, Copy, PartialEq, Eq, Debug)]\n\
         pub struct CpuidRequirement {\n    \
         pub name: &'static str,\n    \
         pub leaf: u32,\n    \
         pub subleaf: u32,\n    \
         pub register: CpuidRegister,\n    \
         pub lowest_bit: u8,\n    \
         pub highest_bit: u8,\n    \
         pub value: u32,\n\
         }\n",
    );
    let requirements: BTreeMap<&str, &CpuidRequirement> = forms
        .iter()
        .flat_map(|form| &form.cpuid)
        .flat_map(|alternative| &alternative.requirements)
        .map(|requirement| (requirement.name.as_str(), requirement))
        .collect();
    for (name, requirement) in &requirements {
        let register = match requirement.register.as_str() {
            "EAX" => "Eax",
            "EBX" => "Ebx",
            "ECX" => "Ecx",
            _ => "Edx",
        };
        module.push_str(&format!(
            "\n\
             pub const {}: CpuidRequirement = CpuidRequirement {{\n    \
             name: \"{name}\",\n    \
             leaf: {:#x},\n    \
             subleaf: {},\n    \
             register: CpuidRegister::{register},\n    \
             lowest_bit: {},\n    \
             highest_bit: {},\n    \
             value: {},\n\
             }};\n",
            to_constant_name(name),
            requirement.leaf,
            requirement.subleaf,
            requirement.lowest_bit,
            requirement.highest_bit,
            requirement.value,
        ));
    }
    module.push_str(&format!(
        "\n\
         /// The CPUID fields each form needs, in the order of `INSTRUCTIONS`: every field of any one alternative; no\n\
         /// alternative for a form XED names no feature for.\n\
         pub static CPUID_REQUIREMENTS: [&[&[CpuidRequirement]]; {}] = [\n",
        forms.len()
    ));
    for form in forms {
        let alternatives: Vec<String> = form
            .cpuid
            .iter()
            .map(|alternative| {
                let names: Vec<String> = alternative
                    .requirements
                    .iter()
                    .map(|requirement| to_constant_name(&requirement.name))
                    .collect();
                format!("&[{}]", names.join(", "))
            })
            .collect();
        push_element(module, &alternatives);
    }
    module.push_str("];\n");
}

/// The widest line rustfmt leaves.
const LINE_WIDTH: usize = 100;

/// Appends one element of a static array of slices, on one line where it fits and else one item a line, as rustfmt
/// leaves it.
fn push_element(module: &mut String, items: &[String]) {
    let line = format!("    &[{}],", items.join(", "));
    if line.len() <= LINE_WIDTH {
        module.push_str(&line);
        module.push('\n');
        return;
    }
    module.push_str("    &[\n");
    for item in items {
        module.push_str(&format!("        {item},\n"));
    }
    module.push_str("    ],\n");
}

/// Appends one sample encoding of each form.
fn render_samples(forms: &[InstructionForm], module: &mut String) {
    module.push_str(&format!(
        "\n\
         /// One encoding of each form, in the order of `INSTRUCTIONS`: distinct registers in the free register fields,\n\
         /// `[rbx]` for memory and zero immediates; empty for a form no pattern encodes in 64-bit mode.\n\
         pub static SAMPLE_ENCODINGS: [&[u8]; {}] = [\n",
        forms.len()
    ));
    for form in forms {
        let bytes = encode_sample(form).map(|(bytes, _)| bytes).unwrap_or_default();
        let bytes: Vec<String> = bytes.iter().map(|byte| format!("{byte:#04X}")).collect();
        push_element(module, &bytes);
    }
    module.push_str("];\n");
}

/// Returns the constant's name of a CPUID requirement: `REQUIRES_` and XED's name in upper case.
fn to_constant_name(name: &str) -> String {
    format!("REQUIRES_{}", name.to_ascii_uppercase())
}
