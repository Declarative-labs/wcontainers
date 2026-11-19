//! Turns NIR, the shader form Mesa's OpenGL and Vulkan drivers receive, into the common form of
//! `wcontainers-gpu` (a validated naga module). Two routes are measured against each other (plan
//! 15.3): NIR to SPIR-V by Zink's compiler then naga's SPIR-V reader (`spirv`), and NIR read straight
//! into naga. The parts that read NIR link the pinned Mesa and are built when `WCONTAINERS_MESA`
//! names its build (`mesa/build`).

pub mod bind;
#[cfg(mesa)]
pub mod mesa;
pub mod spirv;

/// Why a shader did not reach the common form.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConvertError {
    /// Mesa refused the input, at the named step.
    Mesa(&'static str),
    /// naga's SPIR-V reader cannot read what the route wrote.
    Reader(String),
    /// The module failed naga's validation.
    Invalid(String),
    /// The program's resources do not fit the common form's binding rule (plan 6.4), for the named
    /// reason.
    Bindings(&'static str),
}

impl core::fmt::Display for ConvertError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ConvertError::Mesa(step) => write!(f, "mesa refused at {step}"),
            ConvertError::Reader(s) => write!(f, "spirv-reader: {s}"),
            ConvertError::Invalid(s) => write!(f, "invalid: {s}"),
            ConvertError::Bindings(why) => write!(f, "bindings: {why}"),
        }
    }
}

impl std::error::Error for ConvertError {}
