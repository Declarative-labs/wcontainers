//! A container's image files and its start, carried into the inside's kernel through the link before its
//! first process runs.

use alloc::sync::Arc;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};
use wcontainers_abi::Cpu;
use wcontainers_kernel::errno::Errno;
use wcontainers_kernel_linux::{
    ARM64_ADDRESS_SPACE_BYTES, Attributes, FileRecord, Kernel, NewFile, Start, StartOptions,
};
use wcontainers_runtime_single_kernel::System;
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

use crate::link::{Link, Ring};

crate::requests! {
    first: crate::host::Request::END;
    /// The image's next [`File`], by postcard in the answer's bytes.
    NextFile {} -> Length;
    /// The container's [`Starting`], by postcard in the answer's bytes.
    StartContainer {} -> Length;
    /// One file of the root as it is now, a [`FileRecord`] by postcard in the request's bytes.
    RootFile {} -> ();
}

/// Bytes of an answer's record; 0 after the image's last file.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, FromBytes, IntoBytes, Immutable, KnownLayout)]
pub struct Length {
    pub length: u64,
}

/// What a file is, as the kernel's [`NewFile`] says, owning its bytes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileKind {
    Directory,
    Regular(Vec<u8>),
    Symlink(Vec<u8>),
    HardLink(Vec<u8>),
    CharacterDevice { major: u32, minor: u32 },
    BlockDevice { major: u32, minor: u32 },
    Fifo,
}

/// One file of the image, as [`Kernel::add_file`] takes it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct File {
    pub path: Vec<u8>,
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
    pub mtime_seconds: i64,
    pub kind: FileKind,
}

impl File {
    fn new_file(&self) -> NewFile<'_> {
        match &self.kind {
            FileKind::Directory => NewFile::Directory,
            FileKind::Regular(contents) => NewFile::Regular(contents),
            FileKind::Symlink(target) => NewFile::Symlink(target),
            FileKind::HardLink(target) => NewFile::HardLink(target),
            &FileKind::CharacterDevice { major, minor } => {
                NewFile::CharacterDevice { major, minor }
            }
            &FileKind::BlockDevice { major, minor } => NewFile::BlockDevice { major, minor },
            FileKind::Fifo => NewFile::Fifo,
        }
    }
}

/// The container's [`Start`], owning its bytes.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Starting {
    pub path: Vec<u8>,
    pub arguments: Vec<Vec<u8>>,
    pub environment: Vec<Vec<u8>>,
    pub working_directory: Vec<u8>,
    pub hostname: Vec<u8>,
    pub groups: Vec<u32>,
    pub backend: Option<Vec<u8>>,
}

/// The fields of the kernel's [`FileRecord`], for postcard.
#[derive(Serialize, Deserialize)]
#[serde(remote = "FileRecord")]
struct FileRecordFields {
    path: Vec<u8>,
    mode: u32,
    uid: u32,
    gid: u32,
    contents: Vec<u8>,
    rdev: u64,
}

/// A [`FileRecord`] as a [`RootFile`] request carries it.
#[derive(Serialize, Deserialize)]
struct Kept(#[serde(with = "FileRecordFields")] FileRecord);

/// A file the kernel refused, with its path and the kernel's error.
#[derive(Debug)]
pub struct Refused {
    pub path: Vec<u8>,
    pub error: Errno,
}

/// Asks `message`, whose answer is a [`Length`] of a postcard record, and reads the record.
fn record<R: Ring, M: crate::link::Message<Answer = Length>, T: for<'a> Deserialize<'a>>(
    ring: &mut R,
    message: &M,
) -> Option<T> {
    let length = ring.ask(message, &[]).length as usize;
    if length == 0 {
        return None;
    }
    let mut bytes = alloc::vec![0; length];
    ring.take(&mut bytes);
    Some(postcard::from_bytes(&bytes).expect("a record the monitor wrote"))
}

/// Puts every file of the image in `kernel`'s root, then starts the container's first process as a default
/// Docker container's, with its standard descriptors the system's, and tells the system it has started.
pub fn start<R: Ring, S: System>(
    ring: &mut R,
    kernel: &mut Kernel,
    cpu: &mut dyn Cpu,
    system: &Arc<S>,
) -> Result<(), Refused> {
    while let Some(file) = record::<_, _, File>(ring, &NextFile {}) {
        let attributes = Attributes {
            mode: file.mode,
            uid: file.uid,
            gid: file.gid,
            mtime_seconds: file.mtime_seconds,
        };
        kernel
            .add_file(&file.path, attributes, file.new_file())
            .map_err(|error| Refused {
                path: file.path.clone(),
                error,
            })?;
    }
    let starting: Starting = record(ring, &StartContainer {}).expect("the container's start");
    let arguments: Vec<&[u8]> = starting.arguments.iter().map(Vec::as_slice).collect();
    let environment: Vec<&[u8]> = starting.environment.iter().map(Vec::as_slice).collect();
    let start = Start {
        path: &starting.path,
        arguments: &arguments,
        environment: &environment,
        working_directory: &starting.working_directory,
        hostname: &starting.hostname,
        groups: &starting.groups,
        backend: starting.backend.as_deref(),
        standard: system.standard(None),
        address_space_bytes: ARM64_ADDRESS_SPACE_BYTES,
        memory_bytes: cpu.address_space().end,
        ceiling: None,
        options: StartOptions::default(),
    };
    kernel.start(cpu, system, start);
    system.started(kernel, cpu);
    Ok(())
}

/// Sends every file of `kernel`'s root as it is now, after the container has run.
pub fn send_root<R: Ring>(ring: &mut R, kernel: &Kernel, cpu: &mut dyn Cpu) {
    for record in kernel.root_files(cpu) {
        let bytes = postcard::to_allocvec(&Kept(record)).expect("a record postcard can write");
        ring.ask(&RootFile {}, &bytes);
    }
}

/// The monitor's half of a container: the image's files still to send, its start, and the root's files sent
/// back.
pub struct Monitor<'a> {
    pub files: &'a mut dyn Iterator<Item = File>,
    pub starting: &'a Starting,
    pub root: Vec<FileRecord>,
}

impl Monitor<'_> {
    /// Answers `request`, which carried `carried`, giving the answer's bytes.
    pub fn answer(&mut self, request: Request, link: &mut Link, carried: &[u8]) -> Vec<u8> {
        let bytes = match request {
            Request::NextFile => self.files.next().map(|file| postcard::to_allocvec(&file)),
            Request::StartContainer => Some(postcard::to_allocvec(self.starting)),
            Request::RootFile => {
                let Kept(record) =
                    postcard::from_bytes(carried).expect("a record the inside wrote");
                self.root.push(record);
                None
            }
        }
        .map(|record| record.expect("a record postcard can write"))
        .unwrap_or_default();
        link.put(&Length {
            length: bytes.len() as u64,
        });
        bytes
    }
}

#[cfg(test)]
mod tests {
    use alloc::boxed::Box;
    use alloc::vec;
    use alloc::vec::Vec;

    use zerocopy::FromZeros;

    use wcontainers_kernel_linux::FileRecord;

    use super::{
        File, FileKind, Kept, Monitor, NextFile, Request, RootFile, StartContainer, Starting,
        record,
    };
    use crate::link::{LINK_BYTES, Link, Parts, Ring};

    /// The link and the monitor's half in one place, answering from a list of files and a start.
    struct Loopback {
        link: Box<Link>,
        parts: Parts,
        files: vec::IntoIter<File>,
        starting: Starting,
        root: Vec<FileRecord>,
    }

    impl Ring for Loopback {
        fn link(&mut self) -> &mut Link {
            &mut self.link
        }
        fn ring(&mut self) {
            let Some(carried) = self.parts.request(&mut self.link) else {
                return;
            };
            let request = Request::from_word(self.link.head.request).expect("a container request");
            let mut monitor = Monitor {
                files: &mut self.files,
                starting: &self.starting,
                root: core::mem::take(&mut self.root),
            };
            let answer = monitor.answer(request, &mut self.link, &carried);
            self.root = monitor.root;
            self.parts.answer(&mut self.link, answer);
        }
    }

    #[test]
    fn files_and_start_arrive_whole() {
        let file = |path: &str, kind| File {
            path: path.as_bytes().to_vec(),
            mode: 0o640,
            uid: 1,
            gid: 2,
            mtime_seconds: -3,
            kind,
        };
        let files = vec![
            file("/bin", FileKind::Directory),
            file(
                "/bin/busybox",
                FileKind::Regular((0..3 * LINK_BYTES + 5).map(|i| i as u8).collect()),
            ),
            file("/bin/sh", FileKind::Symlink(b"busybox".to_vec())),
            file("/bin/ls", FileKind::HardLink(b"/bin/busybox".to_vec())),
            file(
                wcontainers_kernel_linux::NULL_PATH,
                FileKind::CharacterDevice { major: 1, minor: 3 },
            ),
            file("/dev/loop0", FileKind::BlockDevice { major: 7, minor: 0 }),
            file("/run/fifo", FileKind::Fifo),
            file("/empty", FileKind::Regular(Vec::new())),
        ];
        let starting = Starting {
            path: b"/bin/busybox".to_vec(),
            arguments: vec![b"echo".to_vec(), vec![b'x'; LINK_BYTES]],
            environment: vec![b"PATH=/bin".to_vec()],
            working_directory: b"/".to_vec(),
            hostname: b"host".to_vec(),
            groups: vec![0, 10],
            backend: None,
        };
        let mut ring = Loopback {
            link: Box::new(Link::new_zeroed()),
            parts: Parts::default(),
            files: files.clone().into_iter(),
            starting: starting.clone(),
            root: Vec::new(),
        };
        let mut got = Vec::new();
        while let Some(file) = record::<_, _, File>(&mut ring, &NextFile {}) {
            got.push(file);
        }
        assert_eq!(got, files);
        assert_eq!(
            record::<_, _, Starting>(&mut ring, &StartContainer {}),
            Some(starting)
        );
        let root: Vec<FileRecord> = [0, 1, LINK_BYTES, 3 * LINK_BYTES + 5]
            .iter()
            .map(|&length| FileRecord {
                path: alloc::format!("/file{length}").into_bytes(),
                mode: 0o100644,
                uid: 1,
                gid: 2,
                contents: (0..length).map(|i| i as u8).collect(),
                rdev: 3,
            })
            .collect();
        for record in &root {
            let bytes = postcard::to_allocvec(&Kept(record.clone())).expect("a record");
            ring.ask(&RootFile {}, &bytes);
        }
        assert_eq!(ring.root, root);
    }
}
