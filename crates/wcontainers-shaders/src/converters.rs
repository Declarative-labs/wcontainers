//! The C++ converters (dxbc-spirv, dxil-spirv, with the boundary in `cpp/`), called with a shader's
//! bytes and giving back the buffers they wrote. Natively they run as their own WebAssembly module
//! under wasmtime, so that no shader can end, exhaust or stall the host; the crate's own WebAssembly
//! build links them in.

#[cfg(not(target_os = "wasi"))]
mod module;

#[cfg(target_os = "wasi")]
mod linked;

#[cfg(not(target_os = "wasi"))]
use module as route;

#[cfg(target_os = "wasi")]
use linked as route;

use crate::ir::slot;
use alloc::string::String;
use alloc::vec::Vec;

include!(concat!(env!("OUT_DIR"), "/exports.rs"));

/// The size of one element of a buffer an entry gives.
#[derive(Clone, Copy)]
pub(crate) enum Unit {
    Byte,
    Word,
}

impl Unit {
    fn bytes(self) -> usize {
        match self {
            Unit::Byte => size_of::<u8>(),
            Unit::Word => size_of::<u32>(),
        }
    }
}

/// One function of the boundary, `name(data, size, samples, slots) -> status`, filling the slots of
/// `cpp/boundary.h`'s `Slot`.
pub(crate) struct Entry<const BUFFERS: usize> {
    pub name: &'static str,
    /// The converter it runs, which names it in a refusal.
    pub converter: &'static str,
    /// What each buffer it gives holds, in the order of the slots.
    pub outputs: [Unit; BUFFERS],
}

/// Each buffer's (address, element count) slots.
const BUFFER_SLOTS: [[usize; 2]; 2] = [
    [slot::FIRST, slot::FIRST_COUNT],
    [slot::SECOND, slot::SECOND_COUNT],
];

const DXBC_SPIRV: &str = "dxbc-spirv";
const DXIL_SPIRV: &str = "dxil-spirv";

pub(crate) const DXBC_TO_IR: Entry<2> = Entry {
    name: exports::DXBC_TO_IR,
    converter: DXBC_SPIRV,
    outputs: [Unit::Byte, Unit::Word],
};

pub(crate) const DXIL_TO_SPIRV: Entry<2> = Entry {
    name: exports::DXIL_TO_SPIRV,
    converter: DXIL_SPIRV,
    outputs: [Unit::Word, Unit::Word],
};

/// A reader of the DXIL's module giving one list of words.
const fn dxil_list(name: &'static str) -> Entry<1> {
    Entry {
        name,
        converter: DXIL_SPIRV,
        outputs: [Unit::Word],
    }
}

pub(crate) const DXIL_CONSTANT_SIZES: Entry<1> = dxil_list(exports::DXIL_CONSTANT_SIZES);
pub(crate) const DXIL_COMPARED_GATHER_CHANNELS: Entry<1> =
    dxil_list(exports::DXIL_COMPARED_GATHER_CHANNELS);
pub(crate) const DXIL_DECLARED_REGISTERS: Entry<1> = dxil_list(exports::DXIL_DECLARED_REGISTERS);
pub(crate) const DXIL_VIEWS: Entry<1> = dxil_list(exports::DXIL_VIEWS);

/// Why a call gave nothing.
#[derive(Debug)]
pub(crate) enum Failure {
    /// The converter refused the shader, with its reason (a nonzero status).
    Refused(String),
    /// The converter's run ended before it returned (a trap, its memory bound or its instruction
    /// bound), as the reason says.
    #[cfg_attr(
        target_os = "wasi",
        expect(
            dead_code,
            reason = "linked converters that end, end the module with them"
        )
    )]
    Ended(String),
}

/// The buffers `entry` gives for `data` (with `samples`, which only `DXIL_TO_SPIRV` reads), in the
/// order of `entry.outputs`.
pub(crate) fn call<const BUFFERS: usize>(
    entry: &Entry<BUFFERS>,
    data: &[u8],
    samples: u32,
) -> Result<[Vec<u8>; BUFFERS], Failure> {
    route::call(entry, data, samples)
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

/// The reason given when the converter set no reason.
fn no_reason<const BUFFERS: usize>(entry: &Entry<BUFFERS>) -> String {
    alloc::format!("{} gave no reason", entry.converter)
}
