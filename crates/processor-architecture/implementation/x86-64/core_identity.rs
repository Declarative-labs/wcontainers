use alloc::format;
use alloc::string::String;

/// The bytes of the [vendor identification string](https://www.felixcloutier.com/x86/cpuid) of leaf 0.
const VENDOR_IDENTIFICATION_BYTES: usize = 12;
/// The microcode part of the kind's name where the system reports none.
const MICROCODE_UNREPORTED: &str = "unreported";

/// The design and revision of an x86-64 logical processor as CPUID gives them, and its microcode revision.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct CoreIdentity {
    /// From EBX, EDX and ECX of leaf 0.
    pub vendor_identification: [u8; VENDOR_IDENTIFICATION_BYTES],
    /// DisplayFamily of leaf 1.
    pub family: u16,
    /// DisplayModel of leaf 1.
    pub model: u8,
    pub stepping: u8,
    /// Leaf 0x1A's core type, on a hybrid processor only.
    pub hybrid_core_type: Option<u8>,
    /// What the system reports as loaded; none where it reports none, as in most virtual machines.
    pub microcode_version: Option<u32>,
}

impl CoreIdentity {
    /// Returns the name of the kind of core, which names its baked sample: vendor, family, model, stepping, core type
    /// and microcode, as [CPUID](https://www.felixcloutier.com/x86/cpuid) gives them.
    pub fn to_kind_name(&self) -> String {
        let vendor: String = self
            .vendor_identification
            .iter()
            .filter(|byte| byte.is_ascii_alphanumeric())
            .map(|byte| char::from(byte.to_ascii_lowercase()))
            .collect();
        let core_type = self
            .hybrid_core_type
            .map(|core_type| format!("-core-type-{core_type:02x}"))
            .unwrap_or_default();
        let microcode = self.microcode_version.map_or_else(
            || String::from(MICROCODE_UNREPORTED),
            |version| format!("{version:x}"),
        );
        format!(
            "{vendor}-family-{:x}-model-{:x}-stepping-{:x}{core_type}-microcode-{microcode}",
            self.family, self.model, self.stepping
        )
    }
}

/// Written by hand so that each integer reads in hexadecimal with the digits of its field, as the manual and every tool
/// print it: DisplayFamily and DisplayModel 2 (`06_8FH`), the stepping 1, the core type 2 and the microcode revision 8.
/// The vendor string stays a byte string.
#[cfg(feature = "bake")]
impl databake::Bake for CoreIdentity {
    fn bake(&self, env: &databake::CrateEnv) -> databake::TokenStream {
        /// The digits of each field.
        const BYTE_DIGITS: usize = 2;
        const STEPPING_DIGITS: usize = 1;
        const MICROCODE_DIGITS: usize = 8;
        env.insert(env!("CARGO_CRATE_NAME"));
        let vendor_identification = databake::Bake::bake(&self.vendor_identification, env);
        let family = to_hexadecimal_literal(self.family.into(), BYTE_DIGITS, "u16");
        let model = to_hexadecimal_literal(self.model.into(), BYTE_DIGITS, "u8");
        let stepping = to_hexadecimal_literal(self.stepping.into(), STEPPING_DIGITS, "u8");
        let hybrid_core_type = match self.hybrid_core_type {
            Some(core_type) => {
                let core_type = to_hexadecimal_literal(core_type.into(), BYTE_DIGITS, "u8");
                databake::quote!(Some(#core_type))
            }
            None => databake::quote!(None),
        };
        let microcode_version = match self.microcode_version {
            Some(version) => {
                let version = to_hexadecimal_literal(version, MICROCODE_DIGITS, "u32");
                databake::quote!(Some(#version))
            }
            None => databake::quote!(None),
        };
        databake::quote!(wcontainers_processor_architecture_x86_64::CoreIdentity {
            vendor_identification: #vendor_identification,
            family: #family,
            model: #model,
            stepping: #stepping,
            hybrid_core_type: #hybrid_core_type,
            microcode_version: #microcode_version,
        })
    }
}

/// Returns the integer as a literal of the suffix's type in hexadecimal, with at least these digits.
#[cfg(feature = "bake")]
fn to_hexadecimal_literal(value: u32, digits: usize, suffix: &str) -> databake::TokenStream {
    let literal = format!("{value:#0width$x}{suffix}", width = digits + "0x".len());
    match literal.parse() {
        Ok(literal) => literal,
        Err(_) => unreachable!("a hexadecimal integer with its suffix is one literal"),
    }
}
