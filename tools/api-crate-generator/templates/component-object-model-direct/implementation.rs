//! COM's bookkeeping under every generated `direct` crate: the table of our objects the C side holds, foreign
//! objects held by one counted reference, IUnknown's entries, the promises of C data pointers, and memory the system
//! lends from Map to Unmap. Every function that changes the table of objects asks for a `GlueKey`, which code under
//! `forbid(unsafe_code)` cannot make.
//!
//! The invariants the glue keeps (SPIKE.md): nothing an implementation returns is believed; read-only stays read
//! only; no lock is held while code of either side runs; the bookkeeping is sealed; every length is checked before
//! memory is touched, and every out-parameter is written on every path; one unsafe operation per block, each marked.

#[path = "foreign.rs"]
mod foreign;
#[path = "key.rs"]
mod key;
#[path = "lent.rs"]
mod lent;
#[path = "objects.rs"]
mod objects;
#[path = "pointers.rs"]
mod pointers;
#[path = "unknown.rs"]
mod unknown;

pub use foreign::{ForeignObject, Interface};
pub use key::GlueKey;
pub use lent::{ForeignLent, Shadow};
pub use objects::{
    Address, Counted, Found, Interfaces, Record, Served, Shield, Table, Weak, add_reference, counted, guarded, object,
    query, release, serve,
};
pub use pointers::{read_bytes, read_only_at, writable_at, write_bytes};
pub use unknown::{Absent, ForeignUnknown, Raw, UnknownTable, unknown_table};

use wcontainers_component_object_model::constants::{E_FAIL, S_OK};
use wcontainers_component_object_model::structures::{Guid, HResult};

/// The header's `IID_IUnknown`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/unknwn/nn-unknwn-iunknown).
pub const IID_IUNKNOWN: Guid = Guid { data1: 0, data2: 0, data3: 0, data4: [0xc0, 0, 0, 0, 0, 0, 0, 0x46] };

/// The bytes of a GUID as guiddef.h lays it out.
pub const GUID_SIZE: usize = 16;

/// Returns the status C sees for a result: S_OK, or the implementation's failure, which must be negative (a
/// "failure" that reads as success is answered as E_FAIL).
pub fn status<T>(result: &Result<T, HResult>) -> i32 {
    match result {
        Ok(_) => S_OK.0,
        Err(failure) => failed(*failure).0,
    }
}

/// Returns a failure as C must see it: negative, else E_FAIL.
pub fn failed(failure: HResult) -> HResult {
    if failure.0 < 0 { failure } else { E_FAIL }
}

/// Returns the result of a status a foreign call returned: Ok for success, the failure otherwise.
pub fn succeeded(code: i32) -> Result<(), HResult> {
    if code < 0 { Err(HResult(code)) } else { Ok(()) }
}

/// Reads a GUID from the 16 bytes guiddef.h lays it out in.
pub fn guid_from(bytes: &[u8; GUID_SIZE]) -> Guid {
    let [a, b, c, d, e, f, g, h, data4 @ ..] = *bytes;
    Guid { data1: u32::from_le_bytes([a, b, c, d]), data2: u16::from_le_bytes([e, f]), data3: u16::from_le_bytes([g, h]), data4 }
}

/// Writes a GUID as guiddef.h lays it out.
pub fn guid_bytes(guid: &Guid) -> [u8; GUID_SIZE] {
    let mut bytes = [0_u8; GUID_SIZE];
    bytes[..4].copy_from_slice(&guid.data1.to_le_bytes());
    bytes[4..6].copy_from_slice(&guid.data2.to_le_bytes());
    bytes[6..8].copy_from_slice(&guid.data3.to_le_bytes());
    bytes[8..].copy_from_slice(&guid.data4);
    bytes
}
