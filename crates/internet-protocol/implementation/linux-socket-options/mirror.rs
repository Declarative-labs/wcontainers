//! Copies of Linux uapi structures that zerocopy reads from and writes as bytes, which linux-raw-sys's do not allow.

/// Declares a `#[repr(C)]` copy of a uapi structure that zerocopy can make from bytes.
///
/// It is checked at compile time to have the uapi structure's size and each field at the offset of the uapi field of
/// the same name.
///
/// The crate using it depends on zerocopy; a copy without padding may add `#[derive(zerocopy::IntoBytes)]`.
#[macro_export]
macro_rules! uapi_mirror {
    (
        $(#[$attribute:meta])*
        $visibility:vis struct $name:ident mirrors $uapi:path {
            $($(#[$field_attribute:meta])* $field_visibility:vis $field:ident: $type:ty,)*
        }
    ) => {
        $(#[$attribute])*
        #[repr(C)]
        #[derive(Clone, Copy, zerocopy::FromBytes, zerocopy::KnownLayout, zerocopy::Immutable)]
        $visibility struct $name {
            $($(#[$field_attribute])* $field_visibility $field: $type,)*
        }

        const _: () = {
            assert!(::core::mem::size_of::<$name>() == ::core::mem::size_of::<$uapi>());
            $(assert!(
                ::core::mem::offset_of!($name, $field) == ::core::mem::offset_of!($uapi, $field)
            );)*
        };
    };
}
