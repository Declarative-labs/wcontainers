//! Turns NIR, the shader form Mesa's OpenGL and Vulkan drivers receive, into the common form of
//! `wcontainers-gpu` (a validated naga module): NIR read straight into naga (`direct`, route B of
//! plan 15.3). The parts that read NIR link the pinned Mesa and are built when `WCONTAINERS_MESA`
//! names its build (`mesa/build`).

pub mod bind;
#[cfg(mesa)]
pub mod direct;
#[cfg(mesa)]
pub mod mesa;
pub mod spirv;

/// Why a shader did not reach the common form.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConvertError {
    /// Mesa refused the input, at the named step.
    Mesa(&'static str),
    /// Mesa's GLSL compiler or linker refused the program, with its messages.
    Glsl(String),
    /// The module failed naga's validation.
    Invalid(String),
    /// The program's resources do not fit the common form's binding rule (plan 6.4), for the named
    /// reason.
    Bindings(&'static str),
    /// Candidate B does not take this part of NIR.
    Unsupported(String),
    /// The shaders crate's rewrites (plan 15.4) refused route B's module, with their reason.
    Rewrite(String),
    /// Mesa's compiler gave a number outside the named enumeration of its bridge.
    Unknown(&'static str, u32),
}

/// The variant of `T` (an enumeration of the bridge's, in its order) the compiler gave as `raw`,
/// refused by the enumeration's name where it is none of them.
#[cfg(mesa)]
pub(crate) fn checked<T: zerocopy::TryFromBytes>(
    raw: u32,
    enumeration: &'static str,
) -> Result<T, ConvertError> {
    T::try_read_from_bytes(&raw.to_ne_bytes()).map_err(|_| ConvertError::Unknown(enumeration, raw))
}

impl core::fmt::Display for ConvertError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ConvertError::Mesa(step) => write!(f, "mesa refused at {step}"),
            ConvertError::Glsl(log) => write!(f, "glsl: {log}"),
            ConvertError::Invalid(s) => write!(f, "invalid: {s}"),
            ConvertError::Bindings(why) => write!(f, "bindings: {why}"),
            ConvertError::Unsupported(what) => write!(f, "not taken: {what}"),
            ConvertError::Rewrite(why) => write!(f, "rewrite: {why}"),
            ConvertError::Unknown(enumeration, raw) => {
                write!(f, "mesa gave {raw}, no value of {enumeration}")
            }
        }
    }
}

impl std::error::Error for ConvertError {}
