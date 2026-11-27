//! COM's bookkeeping, written once: the generator copies it into `component-object-model/implementation/direct`,
//! which every generated `direct` crate stands on. Its text is in templates/ beside this program.

use std::path::Path;

use crate::emit::HEADER;

/// The files of the COM crate's `direct` implementation, by name.
const FILES: &[(&str, &str)] = &[
    ("implementation.rs", include_str!("../templates/component-object-model-direct/implementation.rs")),
    ("foreign.rs", include_str!("../templates/component-object-model-direct/foreign.rs")),
    ("key.rs", include_str!("../templates/component-object-model-direct/key.rs")),
    ("lent.rs", include_str!("../templates/component-object-model-direct/lent.rs")),
    ("lent_tests.rs", include_str!("../templates/component-object-model-direct/lent_tests.rs")),
    ("objects.rs", include_str!("../templates/component-object-model-direct/objects.rs")),
    ("pointers.rs", include_str!("../templates/component-object-model-direct/pointers.rs")),
    ("unknown.rs", include_str!("../templates/component-object-model-direct/unknown.rs")),
];

/// The manifest: a `direct` crate denies unsafe code and marks each use.
const MANIFEST: &str = r#"[package]
name = "wcontainers-component-object-model-direct"
description = "COM's bookkeeping under every generated direct crate: our objects as the C side holds them, and the C side's objects as we hold them."
version = "0.1.0"
edition = "2024"
publish = false

[lib]
path = "implementation.rs"

[features]
# A table of objects small enough for a test to fill: 12,288 objects instead of 16,773,120, and a quarantine of 1,024.
small-table = []
# Shadow memory of exactly the length lent, never rounded up to a size class: AddressSanitizer then reports a touch one
# byte past what the glue lends (the exact C tests turn it on).
exact-shadow = []

[dependencies]
wcontainers-byte-sink = { path = "../../../byte-sink" }
wcontainers-byte-source = { path = "../../../byte-source" }
wcontainers-component-object-model = { path = "../.." }
wcontainers-volatile-bytes = { path = "../../../volatile-bytes" }
wcontainers-volatile-bytes-pointer = { path = "../../../volatile-bytes/implementation/pointer" }
# The C library's memcpy, which the copy out of C's read-only bytes calls, as the owner's pointer crate does.
libc = { version = "0.2.191", default-features = false }

# A C boundary: it denies unsafe code, so that each use is marked with its reason.
[lints.rust]
unsafe_code = "deny"
unsafe_op_in_unsafe_fn = "deny"

[lints.clippy]
type_complexity = "allow"
"#;

/// Writes the COM crate's `direct` implementation under `crate_folder`.
pub fn write(crate_folder: &Path) -> Result<(), String> {
    let folder = crate_folder.join("implementation").join("direct");
    std::fs::create_dir_all(&folder).map_err(|error| format!("{}: {error}", folder.display()))?;
    for (name, text) in FILES {
        let written = format!("//! {HEADER}\n//!\n{text}");
        std::fs::write(folder.join(name), written).map_err(|error| format!("{name}: {error}"))?;
    }
    std::fs::write(folder.join("Cargo.toml"), MANIFEST).map_err(|error| error.to_string())
}
