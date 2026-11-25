//! The C++ shader converters dxbc-spirv and dxil-spirv, called with a shader's bytes.
//!
//! Each function gives back the buffers it wrote.

/// One function of the converters' boundary, `name(data, size, flags, slots) -> status` in `cpp/boundary.h`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Function {
    /// DXBC to dxbc-spirv's legalized IR, with the declared size of each constant buffer.
    DxbcToIr,
    /// SPIR-V words from DXIL, with the resources its bindings index.
    DxilToSpirv,
    /// The space, register and size in bytes of every constant buffer a DXIL module declares.
    DxilConstantSizes,
    /// The channel of each comparison gather a DXIL module calls.
    DxilComparedGatherChannels,
    /// Every register a DXIL module's resource list declares.
    DxilDeclaredRegisters,
    /// The views of a DXIL module's resource list.
    DxilViews,
}

/// The reason a call gave nothing.
#[derive(Debug, PartialEq, Eq)]
pub enum Failure {
    /// The converter refused the shader, with its reason.
    Refused(String),
    /// The converter's run ended before it returned, as the reason says.
    ///
    /// It ends by a trap, a memory bound or an instruction bound.
    Ended(String),
}

/// The number of functions of the boundary, one entry of each table below each.
pub const FUNCTIONS: usize = 6;

/// The most buffers one function gives.
pub const BUFFERS: usize = 2;

/// The size of a buffer's element that is a byte.
pub const BYTE: usize = size_of::<u8>();

/// The size of a buffer's element that is a little-endian word.
pub const WORD: usize = size_of::<u32>();

/// The element size of a buffer that a function does not give.
pub const NO_BUFFER: usize = 0;

/// The name each function is exported under (`WCS_EXPORT` in `cpp/`), at the function's place in `Function`.
pub const EXPORT_NAMES: [&str; FUNCTIONS] = [
    "wcs_dxbc_to_ir",
    "wcs_dxil_to_spirv",
    "wcs_dxil_constant_sizes",
    "wcs_dxil_compared_gather_channels",
    "wcs_dxil_declared_registers",
    "wcs_dxil_views",
];

/// The converter each function runs, which names it in a refusal.
pub const CONVERTER_NAMES: [&str; FUNCTIONS] = [
    DXBC_SPIRV, DXIL_SPIRV, DXIL_SPIRV, DXIL_SPIRV, DXIL_SPIRV, DXIL_SPIRV,
];

/// The element size of each buffer each function gives, in slot order.
pub const ELEMENT_SIZES: [[usize; BUFFERS]; FUNCTIONS] = [
    [BYTE, WORD],
    [WORD, WORD],
    [WORD, NO_BUFFER],
    [WORD, NO_BUFFER],
    [WORD, NO_BUFFER],
    [WORD, NO_BUFFER],
];

/// The name of dxbc-spirv in a refusal.
pub const DXBC_SPIRV: &str = "dxbc-spirv";

/// The name of dxil-spirv in a refusal.
pub const DXIL_SPIRV: &str = "dxil-spirv";

/// The name of the boundary's function that frees what another function gave.
pub const FREE: &str = "wcs_free";

/// The name of the boundary's function that allocates in the converters' own memory.
pub const ALLOCATE: &str = "wcs_allocate";

/// The status a boundary function returns when it gave its buffers.
pub const SUCCEEDED: i32 = 0;

/// The flags of a call whose function reads none.
///
/// Only `Function::DxilToSpirv` reads them, as the targets' sample count.
pub const NO_FLAGS: u32 = 0;

/// The slot of the first buffer's address, of those a boundary function fills (`Slot` in `cpp/boundary.h`).
///
/// Each slot is a word: the first buffer's address and element count, the second's, and the reason's address and
/// length.
pub const SLOT_FIRST: usize = 0;

/// The slot of the first buffer's element count.
pub const SLOT_FIRST_COUNT: usize = 1;

/// The slot of the second buffer's address.
pub const SLOT_SECOND: usize = 2;

/// The slot of the second buffer's element count.
pub const SLOT_SECOND_COUNT: usize = 3;

/// The slot of the reason's address.
pub const SLOT_REASON: usize = 4;

/// The slot of the reason's length in bytes.
pub const SLOT_REASON_LENGTH: usize = 5;

/// The number of slots.
pub const SLOT_COUNT: usize = 6;

/// The address and count slots of each buffer, in slot order.
pub const BUFFER_SLOTS: [[usize; 2]; BUFFERS] = [
    [SLOT_FIRST, SLOT_FIRST_COUNT],
    [SLOT_SECOND, SLOT_SECOND_COUNT],
];

/// The buffers one call gave, owned, in slot order; a buffer its function does not give is empty.
pub type Buffers = [Vec<u8>; BUFFERS];

/// The converters, reached by one call a function of their boundary.
///
/// Calls on the same shader while a value lives on the calling thread may share the converters' state; calls on
/// another shader, or while no value lives, share nothing.
pub trait ShaderConverters {
    /// Runs `function` on the shader's `bytes` with `flags`, and returns the buffers it wrote or why it gave none.
    fn call(&self, function: Function, bytes: &[u8], flags: u32) -> Result<Buffers, Failure>;
}

/// The converters as a value of their own, made with no outside state.
pub trait StandaloneShaderConverters: ShaderConverters {
    type Config;

    fn new(config: Self::Config) -> Self;
}
