//! The C++ converters (dxbc-spirv, dxil-spirv, through wcontainers-shader-converters), called with a
//! shader's bytes and giving back the buffers they wrote. They are linked into the crate, natively and in its
//! own WebAssembly build; natively with the feature `contained-converters` they run as their own
//! WebAssembly module under wasmtime instead, so that nothing a shader makes them do can end,
//! exhaust or stall the host: it ends in a trap or a bound of their instance, refused by name.

#[cfg(all(feature = "contained-converters", not(target_os = "wasi")))]
use wcontainers_shader_converters_contained::ContainedShaderConverters as Route;

#[cfg(not(all(feature = "contained-converters", not(target_os = "wasi"))))]
use wcontainers_shader_converters_linked::LinkedShaderConverters as Route;

use alloc::vec::Vec;
use wcontainers_shader_converters::{
    Function, NO_FLAGS, ShaderConverters, StandaloneShaderConverters,
};

pub(crate) use wcontainers_shader_converters::Failure;

/// One function of the boundary that gives `BUFFERS` buffers.
pub(crate) struct Entry<const BUFFERS: usize> {
    pub function: Function,
}

pub(crate) const DXBC_TO_IR: Entry<2> = Entry {
    function: Function::DxbcToIr,
};

pub(crate) const DXIL_TO_SPIRV: Entry<2> = Entry {
    function: Function::DxilToSpirv,
};

pub(crate) const DXIL_CONSTANT_SIZES: Entry<1> = Entry {
    function: Function::DxilConstantSizes,
};
pub(crate) const DXIL_COMPARED_GATHER_CHANNELS: Entry<1> = Entry {
    function: Function::DxilComparedGatherChannels,
};
pub(crate) const DXIL_DECLARED_REGISTERS: Entry<1> = Entry {
    function: Function::DxilDeclaredRegisters,
};
pub(crate) const DXIL_VIEWS: Entry<1> = Entry {
    function: Function::DxilViews,
};

/// The `samples` of a call whose entry does not read it.
pub(crate) const NO_SAMPLES: u32 = NO_FLAGS;

/// The buffers `entry` gives for `data` (with `samples`, which only `DXIL_TO_SPIRV` reads), in the
/// order of its slots.
pub(crate) fn call<const BUFFERS: usize>(
    entry: &Entry<BUFFERS>,
    data: &[u8],
    samples: u32,
) -> Result<[Vec<u8>; BUFFERS], Failure> {
    let buffers = Route::new(()).call(entry.function, data, samples)?;
    let mut buffers = buffers.into_iter();
    Ok(core::array::from_fn(|_| buffers.next().unwrap_or_default()))
}

/// One conversion's run of calls, which may share the converters' state on the same shader until it
/// is dropped (the module route's instance); calls outside one share nothing.
pub(crate) struct Conversion(
    #[expect(dead_code, reason = "held for its drop, which ends the conversion")] Route,
);

impl Conversion {
    pub(crate) fn begin() -> Conversion {
        Conversion(Route::new(()))
    }
}

/// The words of `bytes`, as the converters' little-endian memory holds them.
pub(crate) fn words(bytes: &[u8]) -> Vec<u32> {
    bytes
        .as_chunks::<{ size_of::<u32>() }>()
        .0
        .iter()
        .map(|w| u32::from_le_bytes(*w))
        .collect()
}
