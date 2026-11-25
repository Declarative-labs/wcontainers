//! The instruction module, written from the data file's records.

use super::form::InstructionForm;

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
    module
}
