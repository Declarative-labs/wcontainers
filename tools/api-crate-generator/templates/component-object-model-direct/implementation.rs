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
    Address, Counted, Found, Interfaces, Record, Served, Shield, Table, Weak, add_reference, alive, counted, guarded, object,
    peek, query, release, serve,
};
pub use pointers::{read_bytes, read_only_at, writable_at, write_bytes};
pub use unknown::{ForeignUnknown, Raw, UnknownTable, unknown_table};

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

/// A GUID passed by value, as guiddef.h declares it: C's calling convention passes this structure.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GuidAbi {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data4: [u8; 8],
}

impl GuidAbi {
    /// Returns the GUID it holds.
    pub fn guid(self) -> Guid {
        Guid { data1: self.data1, data2: self.data2, data3: self.data3, data4: self.data4 }
    }

    /// Returns the form C passes `guid` in.
    pub fn of(guid: &Guid) -> GuidAbi {
        GuidAbi { data1: guid.data1, data2: guid.data2, data3: guid.data3, data4: guid.data4 }
    }
}

/// Bytes laid out for C at the largest alignment a structure of these APIs needs.
#[repr(C, align(16))]
pub struct Aligned<const N: usize>(pub [u8; N]);

impl<const N: usize> Default for Aligned<N> {
    fn default() -> Aligned<N> {
        Aligned([0_u8; N])
    }
}

/// What the glue recorded about the objects one object is related to: the one that made it, those made with it in the
/// same call, and those it handed out by index (a swap chain's buffers), each by the role it plays (its kind's name, or
/// the name of what it was handed out as) and index, held without a reference (so that two related objects do not keep
/// each other alive). An object handed out again (a child's device, the immediate context, a back buffer) is answered
/// from here. Roles, not Rust types: one implementation type may serve several kinds.
///
/// The object that made it, known when it is served, is kept apart and never changed, so that the answer most asked for
/// (a child's device) is read with no lock.
#[derive(Default)]
pub struct Relations {
    made_by: Option<(&'static str, Weak)>,
    later: std::sync::Mutex<Vec<(&'static str, u32, Weak)>>,
}

/// Records that our object of type `V` at `address` is related to `related`, which plays `role` for it.
pub fn relate<V: Send + Sync + 'static>(key: GlueKey, address: usize, role: &'static str, related: Weak) {
    relate_at::<V>(key, address, role, 0, related);
}

/// Records that our object of type `V` at `address` handed out `related` as its `index`th `role`.
pub fn relate_at<V: Send + Sync + 'static>(key: GlueKey, address: usize, role: &'static str, index: u32, related: Weak) {
    let Some(found) = object::<V>(key, address) else { return };
    if let Some(relations) = found.part::<Relations>() {
        let mut held = relations.later.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        held.retain(|(other, at, _)| !(*other == role && *at == index));
        held.push((role, index, related));
    }
}

/// Returns a reference to the object that plays `role` for our object of type `V` at `address`, while it lives.
pub fn related<V: Send + Sync + 'static>(key: GlueKey, address: usize, role: &'static str) -> Option<Counted> {
    related_at::<V>(key, address, role, 0)
}

/// Returns a reference to the object that made our object of type `V` at `address`, if it plays `role` and lives,
/// read with no reference to our object taken.
pub fn made_by_of<V: Send + Sync + 'static>(key: GlueKey, address: usize, role: &'static str) -> Option<Counted> {
    let made_by = peek::<V, Relations, Weak>(key, address, |relations| relations.made_by.filter(|(made_role, _)| *made_role == role).map(|(_, weak)| weak))?;
    made_by.upgrade()
}

/// Returns a reference to the `index`th `role` our object of type `V` at `address` handed out, while it lives.
pub fn related_at<V: Send + Sync + 'static>(key: GlueKey, address: usize, role: &'static str, index: u32) -> Option<Counted> {
    related_of(&object::<V>(key, address)?, role, index)
}

/// As `related_at`, for an object the call already found: no second look in the table.
pub fn related_of<V>(found: &Found<V>, role: &'static str, index: u32) -> Option<Counted> {
    let relations = found.part::<Relations>()?;
    if let Some((made_role, made_by)) = &relations.made_by
        && index == 0
        && *made_role == role
        && let Some(counted) = made_by.upgrade()
    {
        return Some(counted);
    }
    let weak = {
        let held = relations.later.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        held.iter().find(|(other, at, _)| *other == role && *at == index).map(|(_, _, weak)| *weak)
    };
    weak?.upgrade()
}

/// What the serving glue lends the C side across a call pair (Map to Unmap), per subresource of one of our objects;
/// kept in the object's record, so that it dies with the object. A subresource is busy (none) while a Map or an Unmap of
/// it is under way, so that no second one comes between; no code of either side runs under its lock.
#[derive(Default)]
pub struct Lendings(std::sync::Mutex<Vec<(u32, Option<Box<dyn core::any::Any + Send + Sync>>)>>);

impl Lendings {
    /// Marks the subresource busy, or returns none when it is lent or busy.
    pub fn reserve(&self, subresource: u32) -> Option<Reservation<'_>> {
        let mut held = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if held.iter().any(|(other, _)| *other == subresource) {
            return None;
        }
        held.push((subresource, None));
        Some(Reservation { lendings: self, subresource })
    }

    /// Takes what is lent for the subresource, leaving it busy until the reservation drops; none when nothing is.
    pub fn begin_end(&self, subresource: u32) -> Option<(Box<dyn core::any::Any + Send + Sync>, Reservation<'_>)> {
        let mut held = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let lent = held.iter_mut().find(|(other, lent)| *other == subresource && lent.is_some())?.1.take()?;
        Some((lent, Reservation { lendings: self, subresource }))
    }
}

/// A subresource held busy, freed when this drops (also on a panic) unless settled as lent.
pub struct Reservation<'a> {
    lendings: &'a Lendings,
    subresource: u32,
}

impl Reservation<'_> {
    /// Records what is lent for the subresource, until a call that ends the lending takes it.
    pub fn settle(self, lent: Box<dyn core::any::Any + Send + Sync>) {
        let refused = {
            let mut held = self.lendings.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            match held.iter_mut().find(|(other, _)| *other == self.subresource) {
                Some(entry) => {
                    entry.1 = Some(lent);
                    None
                }
                None => Some(lent),
            }
        };
        core::mem::forget(self);
        // An implementation's value, dropped after the lock.
        guarded((), move || drop(refused));
    }
}

impl Drop for Reservation<'_> {
    fn drop(&mut self) {
        let removed = {
            let mut held = self.lendings.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            held.iter().position(|(other, _)| *other == self.subresource).map(|at| held.swap_remove(at))
        };
        guarded((), move || drop(removed));
    }
}

/// What the caller passed when an object was made (its description), which the glue answers later instead of asking
/// the implementation.
pub struct Recorded<T>(pub T);

/// The parts a record holds at most: relations, lendings, and what a getter answers from.
const RECORD_PARTS: usize = 4;

/// A new record holding empty relations and lendings, with room for what the glue records.
pub fn record() -> Record {
    let mut parts: Record = Vec::with_capacity(RECORD_PARTS);
    parts.push(Box::new(Relations::default()));
    parts.push(Box::new(Lendings::default()));
    parts
}

/// A new record whose relations already hold `related` as the object's `role` (the object that made it), so that no
/// look in the table is needed after the object is served.
pub fn record_related(role: &'static str, related: Weak) -> Record {
    let mut parts: Record = Vec::with_capacity(RECORD_PARTS);
    parts.push(Box::new(Relations { made_by: Some((role, related)), later: std::sync::Mutex::default() }));
    parts.push(Box::new(Lendings::default()));
    parts
}

/// An object type of a calling glue: adopted from a reference to an object of the C side's of the interface
/// `INTERFACE` names, which a call of another API's glue returns (a swap chain's buffer).
pub trait Adopt: Sized {
    /// The interface the object is asked for.
    const INTERFACE: Guid;

    /// Takes over a reference to an object of `INTERFACE`'s interface, or returns none for null.
    ///
    /// # Safety
    /// `raw` is null or a live object of `INTERFACE`'s interface, one of whose references the caller gives up.
    #[expect(unsafe_code, reason = "the promise that a pointer is a live object of an interface")]
    unsafe fn adopt(raw: Raw) -> Option<Self>;
}
