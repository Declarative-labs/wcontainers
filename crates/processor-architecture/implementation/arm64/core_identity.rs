use alloc::format;
use alloc::string::String;

/// The hexadecimal digits a register of the identity is written with: the 32 bits of MIDR_EL1 whose bits above are
/// RES0, and as many of REVIDR_EL1, as the kind's name writes both.
const REGISTER_DIGITS: usize = 8;

/// The two registers of an ARM64 core that name its design and its revision.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct CoreIdentity {
    /// [MIDR_EL1](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/MIDR-EL1--Main-ID-Register).
    pub main_id_register: u64,
    /// [REVIDR_EL1](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/REVIDR-EL1--Revision-ID-Register).
    pub revision_id_register: u64,
}

impl CoreIdentity {
    /// Returns the name of the kind of core, which names its baked sample: both registers in hexadecimal.
    pub fn to_kind_name(&self) -> String {
        format!(
            "midr-{:0digits$x}-revidr-{:0digits$x}",
            self.main_id_register,
            self.revision_id_register,
            digits = REGISTER_DIGITS
        )
    }
}

/// Written by hand so that each register reads in hexadecimal, as the manual and every tool print it.
#[cfg(feature = "bake")]
impl databake::Bake for CoreIdentity {
    fn bake(&self, env: &databake::CrateEnv) -> databake::TokenStream {
        env.insert(env!("CARGO_CRATE_NAME"));
        let main_id_register = to_hexadecimal_literal(self.main_id_register);
        let revision_id_register = to_hexadecimal_literal(self.revision_id_register);
        databake::quote!(wcontainers_processor_architecture_arm64::CoreIdentity {
            main_id_register: #main_id_register,
            revision_id_register: #revision_id_register,
        })
    }
}

/// Returns the register as a `u64` literal in hexadecimal, with at least the digits of the register.
#[cfg(feature = "bake")]
fn to_hexadecimal_literal(register: u64) -> databake::TokenStream {
    let literal = format!(
        "{register:#0width$x}u64",
        width = REGISTER_DIGITS + "0x".len()
    );
    match literal.parse() {
        Ok(literal) => literal,
        Err(_) => unreachable!("a hexadecimal integer with its suffix is one literal"),
    }
}
