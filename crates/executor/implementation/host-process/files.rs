//! The embedder's reads and writes of the stubs' memfds, by position, so no view of a stub's memory is mapped here
//! and neither the stubs' number nor their sizes bound our address space.

use std::os::fd::BorrowedFd;

use rustix::io::Errno;

/// Copies bytes from `file` at `offset` into `into`; a range past the file's end is `FAULT`.
pub fn read(file: BorrowedFd<'_>, offset: u64, into: &mut [u8]) -> Result<(), Errno> {
    let mut done = 0;
    while done < into.len() {
        match rustix::io::pread(file, &mut into[done..], offset + done as u64) {
            Ok(0) => return Err(Errno::FAULT),
            Ok(read) => done += read,
            Err(Errno::INTR) => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

/// Copies `bytes` into `file` at `offset`.
pub fn write(file: BorrowedFd<'_>, offset: u64, bytes: &[u8]) -> Result<(), Errno> {
    let mut done = 0;
    while done < bytes.len() {
        match rustix::io::pwrite(file, &bytes[done..], offset + done as u64) {
            Ok(0) => return Err(Errno::FAULT),
            Ok(written) => done += written,
            Err(Errno::INTR) => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}
