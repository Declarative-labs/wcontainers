//! Written by tools/api-crate-generator from the Win32 metadata, version 63.0.31, and never by hand.

/// The header's `GUID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/guiddef/ns-guiddef-guid).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Guid {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data4: [u8; crate::constants::GUID_DATA4_LENGTH],
}

/// The header's `HRESULT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/com/structure-of-com-error-codes).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct HResult(pub i32);

/// A COM object of no kind of these APIs, held by one counted reference while any clone lives; equal to another
/// holding the same object (COM's identity, its `IUnknown` address).
#[derive(Clone)]
pub struct Foreign {
    identity: usize,
    object: std::sync::Arc<dyn core::any::Any + Send + Sync>,
}

impl Foreign {
    /// Wraps the glue's own holder of an object, told apart by `identity`.
    pub fn new(
        identity: usize,
        object: std::sync::Arc<dyn core::any::Any + Send + Sync>,
    ) -> Foreign {
        Foreign { identity, object }
    }

    /// Returns the holder, which only the glue that made it can read back.
    pub fn object(&self) -> &(dyn core::any::Any + Send + Sync) {
        &*self.object
    }

    /// Returns the object's identity.
    pub fn identity(&self) -> usize {
        self.identity
    }
}

impl PartialEq for Foreign {
    fn eq(&self, other: &Foreign) -> bool {
        self.identity == other.identity
    }
}

impl Eq for Foreign {}

impl core::hash::Hash for Foreign {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.identity.hash(state);
    }
}

impl core::fmt::Debug for Foreign {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("Foreign")
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Handle(pub usize);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Hmodule(pub usize);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Hwnd(pub usize);

/// The header's `LUID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/ntdef/ns-ntdef-luid).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Luid {
    pub low_part: u32,
    pub high_part: i32,
}

/// The header's `RECT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/windef/ns-windef-rect).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

/// The header's `SIZE`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/windef/ns-windef-size).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Size {
    pub cx: i32,
    pub cy: i32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Hdc(pub usize);

/// The header's `SECURITY_ATTRIBUTES`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/wtypesbase/ns-wtypesbase-security_attributes).
#[derive(Clone, Debug, PartialEq)]
pub struct SecurityAttributes {
    pub n_length: u32,
    pub lp_security_descriptor: Vec<u8>,
    pub b_inherit_handle: bool,
}

/// The header's `POINT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/windef/ns-windef-point).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Hmonitor(pub usize);
