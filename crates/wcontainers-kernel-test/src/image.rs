//! A container's files: an image's file system as `docker export` gives it, with test programs added,
//! loaded into the kernel's root before it starts.

use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;

use wcontainers_kernel::{Attributes, Kernel, NewFile};

/// One file of an image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct File {
    /// From the root, without a leading slash.
    pub path: String,
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
    pub mtime: i64,
    pub kind: Kind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    Directory,
    Regular(Vec<u8>),
    Symlink(Vec<u8>),
    HardLink(String),
    CharacterDevice(u32, u32),
    BlockDevice(u32, u32),
    Fifo,
}

/// Reads the files of a tar archive in order.
pub fn from_tar(bytes: &[u8]) -> Vec<File> {
    let mut archive = tar::Archive::new(bytes);
    let mut files = Vec::new();
    for entry in archive.entries().expect("reading the archive") {
        let mut entry = entry.expect("an archive entry");
        let header = entry.header().clone();
        let path = entry
            .path()
            .expect("an entry path")
            .to_string_lossy()
            .trim_start_matches("./")
            .trim_end_matches('/')
            .to_string();
        let link = || entry_link(&header);
        let kind = match header.entry_type() {
            tar::EntryType::Directory => Kind::Directory,
            tar::EntryType::Regular | tar::EntryType::Continuous => {
                let mut contents = Vec::new();
                std::io::Read::read_to_end(&mut entry, &mut contents).expect("reading a file");
                Kind::Regular(contents)
            }
            tar::EntryType::Symlink => Kind::Symlink(link()),
            tar::EntryType::Link => Kind::HardLink(
                String::from_utf8(link())
                    .expect("a link name")
                    .trim_start_matches("./")
                    .to_string(),
            ),
            tar::EntryType::Char => Kind::CharacterDevice(
                header.device_major().ok().flatten().unwrap_or(0),
                header.device_minor().ok().flatten().unwrap_or(0),
            ),
            tar::EntryType::Block => Kind::BlockDevice(
                header.device_major().ok().flatten().unwrap_or(0),
                header.device_minor().ok().flatten().unwrap_or(0),
            ),
            tar::EntryType::Fifo => Kind::Fifo,
            other => panic!("unexpected archive entry {other:?} at {path}"),
        };
        files.push(File {
            path,
            mode: header.mode().expect("a mode") & 0o7777,
            uid: header.uid().expect("a uid") as u32,
            gid: header.gid().expect("a gid") as u32,
            mtime: header.mtime().expect("a time") as i64,
            kind,
        });
    }
    files
}

fn entry_link(header: &tar::Header) -> Vec<u8> {
    header
        .link_name_bytes()
        .expect("a link target")
        .into_owned()
}

/// The files of the reference image, from `docker export` of a container made from it. Read once.
pub fn reference_image() -> &'static [File] {
    static FILES: OnceLock<Vec<File>> = OnceLock::new();
    FILES.get_or_init(|| {
        let name = format!("wc-kernel-export-{}", std::process::id());
        let created = Command::new("docker")
            .args([
                "create",
                "--platform",
                "linux/arm64",
                "--name",
                &name,
                crate::reference::IMAGE,
            ])
            .output()
            .expect("running docker create");
        assert!(
            created.status.success(),
            "{}",
            String::from_utf8_lossy(&created.stderr)
        );
        let exported = Command::new("docker")
            .args(["export", &name])
            .output()
            .expect("running docker export");
        let _ = Command::new("docker").args(["rm", "-f", &name]).output();
        assert!(
            exported.status.success(),
            "{}",
            String::from_utf8_lossy(&exported.stderr)
        );
        from_tar(&exported.stdout)
    })
}

/// The files Docker makes in every container, whose contents come from the host; left as the image
/// export gives them and never compared.
pub const DOCKER_FILES: &[&str] = &["etc/hostname", "etc/hosts", "etc/resolv.conf", ".dockerenv"];

/// Adds image files to a kernel's root before it starts.
pub fn add_files(kernel: &mut Kernel, files: &[File]) {
    for file in files {
        let attributes = Attributes {
            mode: file.mode,
            uid: file.uid,
            gid: file.gid,
            mtime_seconds: file.mtime,
        };
        let new = match &file.kind {
            Kind::Directory => NewFile::Directory,
            Kind::Regular(contents) => NewFile::Regular(contents),
            Kind::Symlink(target) => NewFile::Symlink(target),
            Kind::HardLink(target) => NewFile::HardLink(target.as_bytes()),
            Kind::CharacterDevice(major, minor) => NewFile::CharacterDevice {
                major: *major,
                minor: *minor,
            },
            Kind::BlockDevice(major, minor) => NewFile::BlockDevice {
                major: *major,
                minor: *minor,
            },
            Kind::Fifo => NewFile::Fifo,
        };
        let path = format!("/{}", file.path);
        if file.path == "etc/hostname" {
            let hostname = format!("{}\n", crate::reference::HOSTNAME);
            kernel
                .add_file(
                    path.as_bytes(),
                    attributes,
                    NewFile::Regular(hostname.as_bytes()),
                )
                .unwrap_or_else(|error| panic!("adding {path}: {error:?}"));
            continue;
        }
        kernel
            .add_file(path.as_bytes(), attributes, new)
            .unwrap_or_else(|error| panic!("adding {path}: {error:?}"));
    }
}

/// The files of a host directory, placed under `at` (as a read-only volume shows them).
pub fn host_directory(directory: &Path, at: &str) -> Vec<File> {
    let mut files = vec![File {
        path: at.to_string(),
        mode: 0o755,
        uid: 0,
        gid: 0,
        mtime: 0,
        kind: Kind::Directory,
    }];
    let mut entries: Vec<_> = std::fs::read_dir(directory)
        .expect("reading the directory")
        .map(|entry| entry.expect("an entry"))
        .collect();
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let metadata = entry.metadata().expect("metadata");
        if !metadata.is_file() {
            continue;
        }
        use std::os::unix::fs::PermissionsExt;
        files.push(File {
            path: format!("{at}/{}", entry.file_name().to_string_lossy()),
            mode: metadata.permissions().mode() & 0o7777,
            uid: 0,
            gid: 0,
            mtime: 0,
            kind: Kind::Regular(std::fs::read(entry.path()).expect("reading a file")),
        });
    }
    files
}

/// The supplementary groups Docker gives root in an image: root's own group, then every group of the
/// image's `/etc/group` that lists root, in file order.
pub fn root_groups(files: &[File]) -> Vec<u32> {
    let mut groups = vec![0];
    let group_file = files.iter().find(|file| file.path == "etc/group");
    if let Some(File {
        kind: Kind::Regular(contents),
        ..
    }) = group_file
    {
        for line in String::from_utf8_lossy(contents).lines() {
            let fields: Vec<&str> = line.split(':').collect();
            if fields.len() == 4 && fields[3].split(',').any(|member| member == "root") {
                groups.push(fields[2].parse().expect("a group number"));
            }
        }
    }
    groups
}
