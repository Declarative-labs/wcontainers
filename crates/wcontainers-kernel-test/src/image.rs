//! A container's files: an image's file system as `docker export` gives it, with test programs added,
//! loaded into the kernel's root before it starts.

use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;

use wcontainers_kernel::{Attributes, Kernel, NewFile, ROOT_GID, ROOT_UID};

/// The image's user and group databases, and how their lines are made: fields apart, a group's
/// members apart, and where a line keeps its id, a group line its members, and how many fields a
/// group line has.
const PASSWD_PATH: &str = "etc/passwd";
const GROUP_PATH: &str = "etc/group";
const FIELD_SEPARATOR: char = ':';
const MEMBER_SEPARATOR: char = ',';
const NAME_FIELD: usize = 0;
const ID_FIELD: usize = 2;
const MEMBERS_FIELD: usize = 3;
const GROUP_FIELDS: usize = MEMBERS_FIELD + 1;

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

/// An image's files as a container starts with them: the files Docker mounts over the image's
/// placeholders have the mode of the host's files, and `/etc/hostname` the container's name.
fn as_started(files: &[File]) -> impl Iterator<Item = File> + '_ {
    files.iter().map(|file| {
        let mut file = file.clone();
        if DOCKER_FILES[..3].contains(&file.path.as_str()) {
            file.mode = 0o644;
        }
        if file.path == "etc/hostname" {
            file.kind = Kind::Regular(format!("{}\n", crate::reference::HOSTNAME).into_bytes());
        }
        if file.path == "etc/hosts" {
            file.kind = Kind::Regular(hosts().into_bytes());
        }
        if file.path == "etc/resolv.conf" {
            file.kind = Kind::Regular(resolver().into_bytes());
        }
        file
    })
}

/// `/etc/hosts` as Docker writes it, for the made-up network's address.
pub(crate) fn hosts() -> String {
    let [a, b, c, d] = wcontainers_kernel::network::ADDRESS;
    format!(
        "127.0.0.1\tlocalhost\n::1\tlocalhost ip6-localhost ip6-loopback\nfe00::\tip6-localnet\n\
         ff00::\tip6-mcastprefix\nff02::1\tip6-allnodes\nff02::2\tip6-allrouters\n\
         {a}.{b}.{c}.{d}\t{}\n",
        crate::reference::HOSTNAME
    )
}

/// `/etc/resolv.conf` naming the kernel's name server, as Docker names its own.
pub(crate) fn resolver() -> String {
    let [a, b, c, d] = wcontainers_kernel::network::NAME_SERVER;
    format!("nameserver {a}.{b}.{c}.{d}\n")
}

/// A container's files as a tar archive, for unpacking into a backend's directory.
pub fn to_tar(files: &[File]) -> Vec<u8> {
    let mut archive = tar::Builder::new(Vec::new());
    for file in as_started(files) {
        let mut header = tar::Header::new_gnu();
        header.set_mode(file.mode);
        header.set_uid(file.uid.into());
        header.set_gid(file.gid.into());
        header.set_mtime(file.mtime.max(0) as u64);
        header.set_size(0);
        let path = if file.path.is_empty() {
            "./".to_string()
        } else {
            format!("./{}", file.path)
        };
        let contents: &[u8] = match &file.kind {
            Kind::Regular(bytes) => {
                header.set_entry_type(tar::EntryType::Regular);
                header.set_size(bytes.len() as u64);
                bytes
            }
            Kind::Directory => {
                header.set_entry_type(tar::EntryType::Directory);
                &[]
            }
            Kind::Symlink(target) => {
                header.set_entry_type(tar::EntryType::Symlink);
                header.set_link_name_literal(target).expect("a link target");
                &[]
            }
            Kind::HardLink(target) => {
                header.set_entry_type(tar::EntryType::Link);
                header
                    .set_link_name_literal(format!("./{target}"))
                    .expect("a link target");
                &[]
            }
            Kind::CharacterDevice(major, minor) | Kind::BlockDevice(major, minor) => {
                header.set_entry_type(if matches!(file.kind, Kind::CharacterDevice(..)) {
                    tar::EntryType::Char
                } else {
                    tar::EntryType::Block
                });
                header.set_device_major(*major).expect("a device");
                header.set_device_minor(*minor).expect("a device");
                &[]
            }
            Kind::Fifo => {
                header.set_entry_type(tar::EntryType::Fifo);
                &[]
            }
        };
        archive
            .append_data(&mut header, path, contents)
            .expect("an archive entry");
    }
    archive.into_inner().expect("the archive")
}

/// Files read back from a tar archive as the kernel's records of a root file system, with hard
/// links given their target's contents.
pub fn records(files: &[File]) -> Vec<wcontainers_kernel::FileRecord> {
    let mut out: Vec<wcontainers_kernel::FileRecord> = Vec::new();
    for file in files.iter().filter(|file| !file.path.is_empty()) {
        let (kind, contents, rdev) = match &file.kind {
            Kind::Directory => (0o040000, Vec::new(), 0),
            Kind::Regular(bytes) => (0o100000, bytes.clone(), 0),
            Kind::Symlink(target) => (0o120000, target.clone(), 0),
            Kind::HardLink(target) => {
                let path = format!("/{target}");
                let target = out.iter().find(|record| record.path == path.as_bytes());
                (
                    0o100000,
                    target
                        .map(|record| record.contents.clone())
                        .unwrap_or_default(),
                    0,
                )
            }
            Kind::CharacterDevice(major, minor) | Kind::BlockDevice(major, minor) => {
                let kind = if matches!(file.kind, Kind::CharacterDevice(..)) {
                    0o020000
                } else {
                    0o060000
                };
                let (major, minor) = (u64::from(*major), u64::from(*minor));
                (
                    kind,
                    Vec::new(),
                    (minor & 0xff) | (major << 8) | ((minor & !0xff) << 12),
                )
            }
            Kind::Fifo => (0o010000, Vec::new(), 0),
        };
        out.push(wcontainers_kernel::FileRecord {
            path: format!("/{}", file.path).into_bytes(),
            mode: kind | file.mode,
            uid: file.uid,
            gid: file.gid,
            contents,
            rdev,
        });
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

/// Adds image files to a kernel's root before it starts.
pub fn add_files(kernel: &mut Kernel, files: &[File]) {
    for file in as_started(files) {
        let file = &file;
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
        kernel
            .add_file(path.as_bytes(), attributes, new)
            .unwrap_or_else(|error| panic!("adding {path}: {error:?}"));
    }
}

/// The files of a host directory and those below it, placed under `at` (as a read-only volume
/// shows them).
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
        // Another test may be building a program here: a file gone since the listing is skipped.
        let metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => panic!("metadata: {error}"),
        };
        let path = format!("{at}/{}", entry.file_name().to_string_lossy());
        if metadata.is_dir() {
            files.extend(host_directory(&entry.path(), &path));
            continue;
        }
        if !metadata.is_file() {
            continue;
        }
        use std::os::unix::fs::PermissionsExt;
        let bytes = match std::fs::read(entry.path()) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => panic!("reading a file: {error}"),
        };
        files.push(File {
            path,
            mode: metadata.permissions().mode() & 0o7777,
            uid: 0,
            gid: 0,
            mtime: 0,
            kind: Kind::Regular(bytes),
        });
    }
    files
}

/// The supplementary groups Docker gives root in an image: root's own group, then every group of the
/// image's `/etc/group` that lists root, in file order.
pub fn root_groups(files: &[File]) -> Vec<u32> {
    user_groups(files, ROOT_UID, ROOT_GID)
}

/// The supplementary groups Docker gives a user and group in an image: the group, then every group
/// of the image's `/etc/group` that lists the user's name in `/etc/passwd`, in file order.
pub fn user_groups(files: &[File], user: u32, group: u32) -> Vec<u32> {
    let lines = |path: &str| -> Vec<Vec<String>> {
        match files.iter().find(|file| file.path == path) {
            Some(File {
                kind: Kind::Regular(contents),
                ..
            }) => String::from_utf8_lossy(contents)
                .lines()
                .map(|line| line.split(FIELD_SEPARATOR).map(str::to_string).collect())
                .collect(),
            _ => Vec::new(),
        }
    };
    let user = user.to_string();
    let name = lines(PASSWD_PATH)
        .into_iter()
        .find(|fields| fields.get(ID_FIELD) == Some(&user))
        .map(|fields| fields[NAME_FIELD].clone());
    let mut groups = vec![group];
    if let Some(name) = name {
        for fields in lines(GROUP_PATH) {
            if fields.len() == GROUP_FIELDS
                && fields[MEMBERS_FIELD]
                    .split(MEMBER_SEPARATOR)
                    .any(|member| *member == name)
            {
                groups.push(fields[ID_FIELD].parse().expect("a group number"));
            }
        }
    }
    groups
}
