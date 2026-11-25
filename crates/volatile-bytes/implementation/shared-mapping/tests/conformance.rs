use wcontainers_volatile_bytes::StandaloneVolatileBytes;
use wcontainers_volatile_bytes_shared_mapping::{Mapped, SharedMapping};

/// Returns fresh anonymous memory of the length.
fn anonymous(length: usize) -> SharedMapping {
    SharedMapping::new(Mapped::anonymous(length).expect("the host maps memory"))
}

mod whole {
    wcontainers_volatile_bytes_conformance::tests!(super::anonymous(u16::MAX as usize));
}

mod part {
    wcontainers_volatile_bytes_conformance::tests!(
        super::anonymous(u16::MAX as usize).part(3..u16::MAX as u64 - 1).expect("inside")
    );
}

#[cfg(all(target_os = "linux", not(miri)))]
mod file {
    use std::os::fd::AsFd;

    use rustix::fs::{MemfdFlags, SealFlags, fcntl_add_seals, ftruncate, memfd_create};

    use super::*;

    fn mapped() -> SharedMapping {
        let file = memfd_create("conformance", MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING).expect("a file");
        ftruncate(&file, u16::MAX as u64).expect("the file grows");
        fcntl_add_seals(&file, SealFlags::SHRINK).expect("the file takes the seal");
        SharedMapping::new(Mapped::file(file.as_fd(), 8, u16::MAX as usize - 9).expect("the host maps the file"))
    }

    wcontainers_volatile_bytes_conformance::tests!(mapped());
}
