//! A container's root files: a plain tar of files, or an image in the OCI image layout.
//!
//! An image's layers are applied in order with their whiteouts, and its configuration becomes the OCI runtime
//! configuration by the image specification's conversion (`conversion.md`).

use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use oci_spec::image::{
    ANNOTATION_CREATED, Descriptor, DigestAlgorithm, ImageConfiguration, ImageIndex, ImageManifest,
    MediaType, OciLayout, Os,
};
use oci_spec::runtime::{Spec, User};
use ruzstd::decoding::errors::{FrameDecoderError, ReadFrameHeaderError};
use ruzstd::decoding::{BlockDecodingStrategy, FrameDecoder};
use sha2::{Digest as _, Sha256};
use wcontainers_kernel_linux::linux::general::{
    S_IRGRP, S_IROTH, S_IRUSR, S_IRWXG, S_IRWXO, S_IRWXU, S_ISGID, S_ISUID, S_ISVTX, S_IWUSR,
    S_IXGRP, S_IXOTH, S_IXUSR,
};
use wcontainers_kernel_linux::{Attributes, Kernel, NewFile};

use crate::Error;

/// One file of the root, as a tar entry gives it.
#[derive(Clone, Debug)]
struct Entry {
    attributes: Attributes,
    kind: Kind,
}

#[derive(Clone, Debug)]
enum Kind {
    Directory,
    Regular(Vec<u8>),
    Symlink(Vec<u8>),
    HardLink(Vec<u8>),
    CharacterDevice { major: u32, minor: u32 },
    BlockDevice { major: u32, minor: u32 },
    Fifo,
}

/// The root's files, and for an OCI image its configuration.
pub struct Image {
    /// The files by path from the root without a leading slash ("" is the root), so a directory comes before what it holds.
    files: BTreeMap<Vec<u8>, Entry>,
    configuration: Option<ImageConfiguration>,
}

/// Symbolic links followed in one lookup before it fails, as Linux's `MAXSYMLINKS`.
///
/// https://github.com/torvalds/linux/blob/master/include/linux/namei.h
const MOST_LINKS: usize = 40;

/// The permission bits of a mode, the rest being its kind.
const PERMISSION_BITS: u32 = S_ISUID | S_ISGID | S_ISVTX | S_IRWXU | S_IRWXG | S_IRWXO;
/// The bits of a mode any of which makes a regular file one that `PATH` finds, as runc's lookup asks.
const EXECUTABLE: u32 = S_IXUSR | S_IXGRP | S_IXOTH;
/// `rwxr-xr-x` and `rw-r--r--`, the modes Docker gives the directory and files it writes.
const DIRECTORY_MODE: u32 = S_IRWXU | S_IRGRP | S_IXGRP | S_IROTH | S_IXOTH;
const FILE_MODE: u32 = S_IRUSR | S_IWUSR | S_IRGRP | S_IROTH;

/// The OCI image layout's marker file, its version, its index and its blobs' directory (image-layout.md).
pub const LAYOUT_FILE: &str = "oci-layout";
const LAYOUT_VERSION: &str = "1.0.0";
pub const INDEX_FILE: &str = "index.json";
pub const BLOBS_DIRECTORY: &str = "blobs";

/// Docker's media type of a gzip-compressed layer, which `docker save` writes.
const DOCKER_LAYER_GZIP: &str = "application/vnd.docker.image.rootfs.diff.tar.gzip";

/// The start of a whiteout's name, and the name of the whiteout that empties its directory (layer.md).
const WHITEOUT_PREFIX: &[u8] = b".wh.";
const OPAQUE_WHITEOUT: &[u8] = b".wh..wh..opq";

/// The start of a PAX header key holding an extended attribute.
const XATTR_PAX_PREFIX: &[u8] = b"SCHILY.xattr.";

/// The annotations conversion.md makes of an image configuration's fields.
mod annotation {
    pub(super) const OS: &str = "org.opencontainers.image.os";
    pub(super) const ARCHITECTURE: &str = "org.opencontainers.image.architecture";
    pub(super) const VARIANT: &str = "org.opencontainers.image.variant";
    pub(super) const OS_VERSION: &str = "org.opencontainers.image.os.version";
    pub(super) const OS_FEATURES: &str = "org.opencontainers.image.os.features";
    pub(super) const AUTHOR: &str = "org.opencontainers.image.author";
    pub(super) const STOP_SIGNAL: &str = "org.opencontainers.image.stopSignal";
    pub(super) const EXPOSED_PORTS: &str = "org.opencontainers.image.exposedPorts";
}

/// What joins the items of a list in an annotation (conversion.md).
const LIST_SEPARATOR: &str = ",";

/// The image configuration's fields this runtime refuses values of, as the refusals name them.
const CONFIG_VOLUMES: &str = "config.Volumes";
const CONFIG_USER: &str = "config.User";

/// The account tables in the root (passwd(5), group(5)), what separates fields and members, and what starts a comment.
pub const PASSWD: &str = "etc/passwd";
pub const GROUP: &str = "etc/group";
pub const FIELD_SEPARATOR: char = ':';
pub const MEMBER_SEPARATOR: char = ',';
const COMMENT_START: char = '#';

/// The user and group id of root.
///
/// https://man7.org/linux/man-pages/man7/credentials.7.html
pub(crate) const ROOT_ID: u32 = 0;

/// The time the files Docker writes are dated at, the epoch, so that a run repeats.
///
/// https://man7.org/linux/man-pages/man2/time.2.html
const EPOCH_SECONDS: i64 = 0;

/// The root directory, and what separates the parts of a path.
pub(crate) const ROOT_PATH: &str = "/";
pub(crate) const SEPARATOR: char = '/';
pub(crate) const SEPARATOR_BYTE: u8 = SEPARATOR as u8;

/// The parts of a path that name the directory itself and its parent.
const CURRENT_PART: &[u8] = b".";
const PARENT_PART: &[u8] = b"..";

/// Why a hard link or symbolic link entry of a layer is refused.
const LINK_WITHOUT_TARGET: &str = "a link without a target";
const LINK_OUT_OF_ROOT: &str = "a link out of the root";

/// What separates the user from the group in an image configuration's `User` (config.md).
const USER_GROUP_SEPARATOR: char = ':';

/// The directory Docker writes its files in, and those files: the host name, the hosts file and the name server's.
const ETC: &[u8] = b"etc";
const HOSTNAME_FILE: &[u8] = b"etc/hostname";
const HOSTS_FILE: &[u8] = b"etc/hosts";
const RESOLV_CONF: &[u8] = b"etc/resolv.conf";

/// The assignment of the program search path, what separates its directories, and what separates a name from its value.
const PATH_ASSIGNMENT: &str = "PATH=";
const PATH_LIST_SEPARATOR: char = ':';
const ASSIGNMENT: char = '=';

/// Fields of a line of `/etc/passwd` and of `/etc/group`, by their places.
mod field {
    /// The name, first in both tables.
    ///
    /// https://man7.org/linux/man-pages/man5/passwd.5.html
    pub(super) const NAME: usize = 0;
    /// The user id, after the name and the password.
    ///
    /// https://man7.org/linux/man-pages/man5/passwd.5.html
    pub(super) const UID: usize = 2;
    /// The group id, after the user id.
    ///
    /// https://man7.org/linux/man-pages/man5/passwd.5.html
    pub(super) const GID: usize = 3;
    /// The home directory, after the comment.
    ///
    /// https://man7.org/linux/man-pages/man5/passwd.5.html
    pub(super) const HOME: usize = 5;
    /// A group's id, after its name and password.
    ///
    /// https://man7.org/linux/man-pages/man5/group.5.html
    pub(super) const GROUP_GID: usize = 2;
    /// A group's members, after its id.
    ///
    /// https://man7.org/linux/man-pages/man5/group.5.html
    pub(super) const GROUP_MEMBERS: usize = 3;
}

impl Image {
    /// Returns the files of a plain tar of the root, such as `docker export` writes.
    pub fn tar(path: &Path) -> Result<Self, Error> {
        let mut files = BTreeMap::new();
        read_layer(open(path)?, path, false, &mut files)?;
        Ok(Self {
            files,
            configuration: None,
        })
    }

    /// Returns an image in the OCI image layout, as `docker save` writes it, for Linux on `architecture`.
    ///
    /// The architecture is as the image configuration names it ([`crate::Executor::ARCHITECTURE`]); blobs are checked
    /// against their digests and layers applied in order.
    pub fn oci(directory: &Path, architecture: &str) -> Result<Self, Error> {
        let layout = directory.join(LAYOUT_FILE);
        let header = OciLayout::from_reader(open(&layout)?)
            .map_err(|error| Error::Image(format!("{}: {error}", layout.display())))?;
        if header.image_layout_version() != LAYOUT_VERSION {
            return Err(Error::Image(format!(
                "{}: image layout version {} is not {LAYOUT_VERSION}",
                layout.display(),
                header.image_layout_version()
            )));
        }
        let index_path = directory.join(INDEX_FILE);
        let index = ImageIndex::from_reader(open(&index_path)?)
            .map_err(|error| Error::Image(format!("{}: {error}", index_path.display())))?;
        let manifest = find_manifest(directory, index.manifests(), architecture)?;
        let config_bytes = blob(directory, manifest.config())?;
        let configuration = ImageConfiguration::from_reader(config_bytes.as_slice())
            .map_err(|error| Error::Image(format!("the image configuration: {error}")))?;
        if *configuration.os() != Os::Linux
            || configuration.architecture().to_string() != architecture
        {
            return Err(Error::Image(format!(
                "the image is for {}/{}, not linux/{architecture}",
                configuration.os(),
                configuration.architecture()
            )));
        }
        let mut files = BTreeMap::new();
        for layer in manifest.layers() {
            let bytes = blob(directory, layer)?;
            apply_layer(layer, &bytes, &blob_path(directory, layer), &mut files)?;
        }
        Ok(Self {
            files,
            configuration: Some(configuration),
        })
    }

    /// Returns the architecture an OCI image is for; none for a plain tar.
    pub fn architecture(&self) -> Option<String> {
        self.configuration
            .as_ref()
            .map(|configuration| configuration.architecture().to_string())
    }

    /// Returns what to run: the base configuration ([`crate::base_spec`]) with an OCI image's configuration converted in.
    ///
    /// For a plain tar the caller sets `process.args`. Where the image names no user, the user is root with the groups
    /// `/etc/group` lists it in, as Docker gives it.
    pub fn spec(&self) -> Result<Spec, Error> {
        let mut spec = crate::base_spec();
        let mut process = spec.process().clone().unwrap_or_default();
        process.set_user(self.root());
        spec.set_process(Some(process.clone()));
        let Some(configuration) = &self.configuration else {
            return Ok(spec);
        };
        // Implicit annotations first, so that a label of the same key takes precedence.
        let mut annotations: HashMap<String, String> = [
            (annotation::OS, Some(configuration.os().to_string())),
            (
                annotation::ARCHITECTURE,
                Some(configuration.architecture().to_string()),
            ),
            (annotation::VARIANT, configuration.variant().clone()),
            (annotation::OS_VERSION, configuration.os_version().clone()),
            (
                annotation::OS_FEATURES,
                configuration
                    .os_features()
                    .as_ref()
                    .map(|f| f.join(LIST_SEPARATOR)),
            ),
            (annotation::AUTHOR, configuration.author().clone()),
        ]
        .into_iter()
        .filter_map(|(name, value)| Some((name.to_string(), value?)))
        .chain(
            configuration
                .created()
                .clone()
                .map(|created| (ANNOTATION_CREATED.to_string(), created)),
        )
        .collect();
        if let Some(config) = configuration.config() {
            if let Some(volumes) = config.volumes().as_ref().filter(|v| !v.is_empty()) {
                return Err(Error::Refused {
                    field: CONFIG_VOLUMES.to_string(),
                    reason: format!(
                        "{volumes:?}: a volume is a mount of a directory from outside, which needs \
                         the local system's file backend"
                    ),
                });
            }
            let mut args: Vec<String> = config.entrypoint().clone().unwrap_or_default();
            args.extend(config.cmd().clone().unwrap_or_default());
            process.set_args(Some(args));
            process.set_env(Some(with_defaults(
                process.env().clone().unwrap_or_default(),
                config.env().clone().unwrap_or_default(),
            )));
            let cwd = config.working_dir().clone().filter(|cwd| !cwd.is_empty());
            process.set_cwd(PathBuf::from(cwd.unwrap_or_else(|| ROOT_PATH.to_string())));
            if let Some(user) = config.user().as_deref().filter(|user| !user.is_empty()) {
                process.set_user(self.user(user)?);
            }
            if let Some(signal) = config.stop_signal() {
                annotations.insert(annotation::STOP_SIGNAL.to_string(), signal.clone());
            }
            if let Some(ports) = config.exposed_ports().as_ref().filter(|p| !p.is_empty()) {
                annotations.insert(
                    annotation::EXPOSED_PORTS.to_string(),
                    ports.join(LIST_SEPARATOR),
                );
            }
            annotations.extend(config.labels().clone().unwrap_or_default());
        }
        spec.set_process(Some(process));
        spec.set_annotations(Some(annotations));
        Ok(spec)
    }

    /// Returns `process.user` for the image configuration's `User`, its names looked up in `/etc/passwd` and `/etc/group`.
    ///
    /// It takes `user`, `uid`, `user:group`, `uid:gid` and the mixed forms.
    fn user(&self, text: &str) -> Result<User, Error> {
        let passwd = self.table(PASSWD);
        let group = self.table(GROUP);
        let (name, group_name) = match text.split_once(USER_GROUP_SEPARATOR) {
            Some((name, group)) => (name, Some(group)),
            None => (text, None),
        };
        let refused = |reason: String| Error::Refused {
            field: CONFIG_USER.to_string(),
            reason,
        };
        let account = passwd.iter().find(|fields| {
            fields.get(field::NAME) == Some(&name)
                || name.parse::<u32>().is_ok_and(|uid| {
                    fields.get(field::UID).and_then(|f| f.parse().ok()) == Some(uid)
                })
        });
        let uid = match name.parse::<u32>() {
            Ok(uid) => uid,
            Err(_) => account
                .and_then(|fields| fields.get(field::UID)?.parse().ok())
                .ok_or_else(|| refused(format!("no user {name} in the image's {PASSWD}")))?,
        };
        let gid = match group_name {
            Some(group_name) => match group_name.parse::<u32>() {
                Ok(gid) => gid,
                Err(_) => group
                    .iter()
                    .find(|fields| fields.get(field::NAME) == Some(&group_name))
                    .and_then(|fields| fields.get(field::GROUP_GID)?.parse().ok())
                    .ok_or_else(|| {
                        refused(format!("no group {group_name} in the image's {GROUP}"))
                    })?,
            },
            None => account
                .and_then(|fields| fields.get(field::GID)?.parse().ok())
                .unwrap_or(ROOT_ID),
        };
        let mut user = User::default();
        user.set_uid(uid);
        user.set_gid(gid);
        // Groups only for a user named without a group, as conversion.md says.
        if group_name.is_none() && name.parse::<u32>().is_err() {
            user.set_additional_gids(Some(self.groups(name, gid)));
        }
        Ok(user)
    }

    /// Returns the user where the image names none: root, in its groups, as Docker gives it.
    fn root(&self) -> User {
        let passwd = self.table(PASSWD);
        let account = passwd
            .iter()
            .find(|fields| fields.get(field::UID).and_then(|f| f.parse().ok()) == Some(ROOT_ID));
        let gid = account
            .and_then(|fields| fields.get(field::GID)?.parse().ok())
            .unwrap_or(ROOT_ID);
        let mut user = User::default();
        user.set_gid(gid);
        let name = account.and_then(|fields| fields.get(field::NAME).copied());
        user.set_additional_gids(Some(
            name.map_or_else(|| vec![gid], |name| self.groups(name, gid)),
        ));
        user
    }

    /// Returns the groups of the user `name` of group `gid`, as Docker gives them: `gid`, then those listing it in order.
    fn groups(&self, name: &str, gid: u32) -> Vec<u32> {
        let members = self
            .table(GROUP)
            .into_iter()
            .filter(|fields| {
                fields
                    .get(field::GROUP_MEMBERS)
                    .is_some_and(|members| members.split(MEMBER_SEPARATOR).any(|m| m == name))
            })
            .filter_map(|fields| fields.get(field::GROUP_GID)?.parse().ok());
        std::iter::once(gid).chain(members).collect()
    }

    /// Returns the home directory of `uid` in the root's `/etc/passwd`, or `/` as runc has it.
    pub(crate) fn home(&self, uid: u32) -> String {
        self.table(PASSWD)
            .iter()
            .find(|fields| fields.get(field::UID).and_then(|f| f.parse().ok()) == Some(uid))
            .and_then(|fields| fields.get(field::HOME))
            .filter(|home| !home.is_empty())
            .map_or_else(|| ROOT_PATH.to_string(), |home| home.to_string())
    }

    /// Returns the lines of a colon-separated table in the root, such as `/etc/passwd`, split into fields.
    fn table(&self, path: &str) -> Vec<Vec<&str>> {
        match self.lookup(path.as_bytes()) {
            Some(Entry {
                kind: Kind::Regular(bytes),
                ..
            }) => std::str::from_utf8(bytes)
                .unwrap_or("")
                .lines()
                .filter(|line| !line.starts_with(COMMENT_START))
                .map(|line| line.split(FIELD_SEPARATOR).collect())
                .collect(),
            _ => Vec::new(),
        }
    }

    /// Returns the file at `path` (from the root, with `/` between parts), following symbolic links.
    fn lookup(&self, path: &[u8]) -> Option<&Entry> {
        let mut parts: Vec<Vec<u8>> = split(path).rev().map(<[u8]>::to_vec).collect();
        let mut at: Vec<Vec<u8>> = Vec::new();
        let mut links = usize::MIN;
        while let Some(part) = parts.pop() {
            if part == CURRENT_PART {
                continue;
            }
            if part == PARENT_PART {
                at.pop();
                continue;
            }
            at.push(part);
            let entry = self.files.get(&at.join(&SEPARATOR_BYTE))?;
            if let Kind::Symlink(target) = &entry.kind {
                links += 1;
                if links > MOST_LINKS {
                    return None;
                }
                at.pop();
                if target.starts_with(&[SEPARATOR_BYTE]) {
                    at.clear();
                }
                parts.extend(split(target).rev().map(<[u8]>::to_vec));
            } else if !parts.is_empty() && !matches!(entry.kind, Kind::Directory) {
                return None;
            }
        }
        self.files.get(&at.join(&SEPARATOR_BYTE))
    }

    /// Returns the program `process.args[0]` names, as runc finds it, or none for a name with no slash found nowhere.
    ///
    /// A name with a slash is from the working directory, and the kernel's to fail as exec does; any other is the first
    /// file anyone may run in the directories of `PATH` in `env`.
    pub(crate) fn program(&self, name: &str, env: &[String], cwd: &str) -> Option<String> {
        if name.contains(SEPARATOR) {
            return Some(if name.starts_with(SEPARATOR) {
                name.to_string()
            } else {
                format!("{}{SEPARATOR}{name}", cwd.trim_end_matches(SEPARATOR))
            });
        }
        let path = env
            .iter()
            .rev()
            .find_map(|variable| variable.strip_prefix(PATH_ASSIGNMENT))
            .unwrap_or("");
        path.split(PATH_LIST_SEPARATOR)
            .filter(|directory| !directory.is_empty())
            .map(|directory| format!("{}{SEPARATOR}{name}", directory.trim_end_matches(SEPARATOR)))
            .find(|candidate| {
                matches!(
                    self.lookup(candidate.as_bytes()),
                    Some(Entry { attributes, kind: Kind::Regular(_) }) if attributes.mode & EXECUTABLE != 0
                )
            })
    }

    /// Puts the files in a new kernel's root, directories first and hard links last, with Docker's own files in place.
    pub(crate) fn add_to(&self, kernel: &mut Kernel, hostname: &str) -> Result<(), Error> {
        let written = docker_files(hostname);
        let etc = match self.files.get(ETC) {
            None => Some((ETC.to_vec(), docker_file(DIRECTORY_MODE, Kind::Directory))),
            Some(Entry {
                kind: Kind::Directory,
                ..
            }) => None,
            Some(_) => {
                return Err(Error::Image(format!(
                    "the image's {} is not a directory, where the host name and name server files go",
                    ETC.escape_ascii()
                )));
            }
        };
        let hard_links = self
            .files
            .iter()
            .filter(|(_, entry)| matches!(entry.kind, Kind::HardLink(_)));
        let others = self.files.iter().filter(|(path, entry)| {
            !matches!(entry.kind, Kind::HardLink(_))
                && !written.iter().any(|(ours, _)| ours == *path)
        });
        let added = etc
            .iter()
            .chain(&written)
            .map(|(path, entry)| (path, entry));
        for (path, entry) in others.chain(added).chain(hard_links) {
            let file = match &entry.kind {
                Kind::Directory => NewFile::Directory,
                Kind::Regular(bytes) => NewFile::Regular(bytes),
                Kind::Symlink(target) => NewFile::Symlink(target),
                Kind::HardLink(target) => NewFile::HardLink(target),
                &Kind::CharacterDevice { major, minor } => {
                    NewFile::CharacterDevice { major, minor }
                }
                &Kind::BlockDevice { major, minor } => NewFile::BlockDevice { major, minor },
                Kind::Fifo => NewFile::Fifo,
            };
            kernel
                .add_file(path, entry.attributes, file)
                .map_err(|error| Error::FileRefused {
                    path: Path::new(ROOT_PATH).join(String::from_utf8_lossy(path).as_ref()),
                    error,
                })?;
        }
        Ok(())
    }
}

/// The image's own configuration converted, with a field the runtime cannot honour named as the refusal's.
impl wcontainers_runtime::Image for Image {
    fn to_runtime_spec(&self) -> Result<Spec, wcontainers_runtime::Refusal> {
        self.spec().map_err(|error| match error {
            Error::Refused { field, reason } => wcontainers_runtime::Refusal { field, reason },
            other => wcontainers_runtime::Refusal {
                field: String::new(),
                reason: other.to_string(),
            },
        })
    }
}

/// Returns what Docker writes over its mounts in every container: the host name, the hosts file and the name server.
fn docker_files(hostname: &str) -> Vec<(Vec<u8>, Entry)> {
    let address = std::net::Ipv4Addr::from(wcontainers_kernel_linux::network::ADDRESS);
    let name_server = std::net::Ipv4Addr::from(wcontainers_kernel_linux::network::NAME_SERVER);
    let (v4, v6) = (std::net::Ipv4Addr::LOCALHOST, std::net::Ipv6Addr::LOCALHOST);
    let hosts = format!(
        "{v4}\tlocalhost\n{v6}\tlocalhost ip6-localhost ip6-loopback\nfe00::\tip6-localnet\n\
         ff00::\tip6-mcastprefix\nff02::1\tip6-allnodes\nff02::2\tip6-allrouters\n\
         {address}\t{hostname}\n"
    );
    let file = |bytes: String| docker_file(FILE_MODE, Kind::Regular(bytes.into_bytes()));
    vec![
        (HOSTNAME_FILE.to_vec(), file(format!("{hostname}\n"))),
        (HOSTS_FILE.to_vec(), file(hosts)),
        (
            RESOLV_CONF.to_vec(),
            file(format!("nameserver {name_server}\n")),
        ),
    ]
}

/// Returns an entry owned by root and dated at the epoch, so that a run repeats.
fn docker_file(mode: u32, kind: Kind) -> Entry {
    Entry {
        attributes: Attributes {
            mode,
            uid: ROOT_ID,
            gid: ROOT_ID,
            mtime_seconds: EPOCH_SECONDS,
        },
        kind,
    }
}

/// Returns the parts of a path, without empty ones.
fn split(path: &[u8]) -> impl DoubleEndedIterator<Item = &[u8]> {
    path.split(|byte| *byte == SEPARATOR_BYTE)
        .filter(|part| !part.is_empty())
}

fn open(path: &Path) -> Result<File, Error> {
    File::open(path).map_err(|error| Error::Io {
        path: path.to_path_buf(),
        error,
    })
}

fn blob_path(directory: &Path, descriptor: &Descriptor) -> PathBuf {
    let digest = descriptor.digest();
    directory
        .join(BLOBS_DIRECTORY)
        .join(digest.algorithm().to_string())
        .join(digest.digest())
}

/// Returns a blob's bytes, checked against its descriptor's size and digest.
fn blob(directory: &Path, descriptor: &Descriptor) -> Result<Vec<u8>, Error> {
    let path = blob_path(directory, descriptor);
    let mut bytes = Vec::new();
    open(&path)?
        .read_to_end(&mut bytes)
        .map_err(|error| Error::Io {
            path: path.clone(),
            error,
        })?;
    let digest = descriptor.digest();
    if *digest.algorithm() != DigestAlgorithm::Sha256 {
        return Err(Error::Image(format!(
            "{digest}: only sha256 digests are checked by this runtime"
        )));
    }
    let found = format!("{:x}", Sha256::digest(&bytes));
    if found != digest.digest() || bytes.len() as u64 != descriptor.size() {
        return Err(Error::Image(format!(
            "{}: its bytes do not match its descriptor (sha256 {found}, {} bytes)",
            path.display(),
            bytes.len()
        )));
    }
    Ok(bytes)
}

/// Returns the manifest for linux and `architecture` among an index's descriptors, through nested indexes.
fn find_manifest(
    directory: &Path,
    descriptors: &[Descriptor],
    architecture: &str,
) -> Result<ImageManifest, Error> {
    let fits = |descriptor: &Descriptor| {
        descriptor.platform().as_ref().is_none_or(|platform| {
            *platform.os() == Os::Linux && platform.architecture().to_string() == architecture
        })
    };
    for descriptor in descriptors.iter().filter(|d| fits(d)) {
        match descriptor.media_type() {
            MediaType::ImageManifest => {
                let bytes = blob(directory, descriptor)?;
                let manifest = ImageManifest::from_reader(bytes.as_slice())
                    .map_err(|error| Error::Image(format!("{}: {error}", descriptor.digest())))?;
                // A manifest with no platform is taken when its configuration is an image's: an
                // attestation's is not.
                if *manifest.config().media_type() == MediaType::ImageConfig {
                    return Ok(manifest);
                }
            }
            MediaType::ImageIndex => {
                let bytes = blob(directory, descriptor)?;
                let index = ImageIndex::from_reader(bytes.as_slice())
                    .map_err(|error| Error::Image(format!("{}: {error}", descriptor.digest())))?;
                if let Ok(manifest) = find_manifest(directory, index.manifests(), architecture) {
                    return Ok(manifest);
                }
            }
            _ => {}
        }
    }
    Err(Error::Image(format!(
        "{}: no image manifest for linux/{architecture}",
        directory.display()
    )))
}

/// Applies the layer `layer` describes, whose blob `name` holds `bytes`, by its media type.
fn apply_layer(
    layer: &Descriptor,
    bytes: &[u8],
    name: &Path,
    files: &mut BTreeMap<Vec<u8>, Entry>,
) -> Result<(), Error> {
    match layer.media_type() {
        MediaType::ImageLayer | MediaType::ImageLayerNonDistributable => {
            read_layer(bytes, name, true, files)
        }
        MediaType::ImageLayerGzip | MediaType::ImageLayerNonDistributableGzip => {
            read_layer(flate2::read::GzDecoder::new(bytes), name, true, files)
        }
        MediaType::Other(other) if other == DOCKER_LAYER_GZIP => {
            read_layer(flate2::read::GzDecoder::new(bytes), name, true, files)
        }
        MediaType::ImageLayerZstd | MediaType::ImageLayerNonDistributableZstd => {
            let mut stream = Zstd::new(bytes);
            read_layer(&mut stream, name, true, files)?;
            // The rest of the stream after the tar's end, so that its last checksums are checked.
            std::io::copy(&mut stream, &mut std::io::sink())
                .map(drop)
                .map_err(|error| Error::Image(format!("{}: {error}", name.display())))
        }
        other => Err(Error::Image(format!(
            "layer {} has media type {other}, which this runtime does not unpack",
            layer.digest()
        ))),
    }
}

/// A reader of a zstd stream's bytes, decompressed as libzstd does (RFC 8878).
///
/// Its frames come one after another, skippable frames passed over, each frame's checksum and declared size checked.
struct Zstd<'a> {
    source: &'a [u8],
    decoder: FrameDecoder,
    /// Bytes given of the frame being read, while one is.
    in_frame: Option<u64>,
    /// Whether the frame being read declares its size.
    declares_size: bool,
    /// Whether a frame, skippable or not, has begun: a stream of none is cut.
    begun: bool,
}

/// Where a zstd frame's header descriptor is, after the four bytes of its magic number.
///
/// https://www.rfc-editor.org/rfc/rfc8878#section-3.1.1
const ZSTD_DESCRIPTOR_AT: usize = 4;
/// The header descriptor's content size field's size.
///
/// https://www.rfc-editor.org/rfc/rfc8878#section-3.1.1.1.1
const ZSTD_CONTENT_SIZE_FLAG: u8 = 0b1100_0000;
/// The header descriptor's single segment flag.
///
/// https://www.rfc-editor.org/rfc/rfc8878#section-3.1.1.1.1
const ZSTD_SINGLE_SEGMENT: u8 = 0b0010_0000;
/// The header descriptor's bit that must be clear.
///
/// https://www.rfc-editor.org/rfc/rfc8878#section-3.1.1.1.1
const ZSTD_RESERVED: u8 = 0b0000_1000;

impl<'a> Zstd<'a> {
    fn new(source: &'a [u8]) -> Self {
        Self {
            source,
            decoder: FrameDecoder::new(),
            in_frame: None,
            declares_size: false,
            begun: false,
        }
    }

    /// Checks what the decoder does not of the header descriptor of the frame `header` starts, which it has just read.
    fn begin_frame(&mut self, header: &[u8]) -> std::io::Result<()> {
        let descriptor = header[ZSTD_DESCRIPTOR_AT];
        if descriptor & ZSTD_RESERVED != 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "a zstd frame sets a reserved bit",
            ));
        }
        self.declares_size = descriptor & (ZSTD_CONTENT_SIZE_FLAG | ZSTD_SINGLE_SEGMENT) != 0;
        self.in_frame = Some(u64::MIN);
        Ok(())
    }

    /// Checks the frame just read to its end.
    fn end_frame(&self, given: u64) -> std::io::Result<()> {
        if let Some(expected) = self.decoder.get_checksum_from_data()
            && self.decoder.get_calculated_checksum() != Some(expected)
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "a zstd frame's checksum does not match its contents",
            ));
        }
        if self.declares_size && self.decoder.content_size() != given {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "a zstd frame's contents are not of the size it declares",
            ));
        }
        Ok(())
    }
}

impl Read for Zstd<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        if buffer.is_empty() {
            return Ok(usize::MIN);
        }
        loop {
            if let Some(given) = self.in_frame {
                // Each step decodes blocks until they hold as much as the caller can take, or the frame ends.
                while self.decoder.can_collect() == 0 && !self.decoder.is_finished() {
                    self.decoder
                        .decode_blocks(
                            &mut self.source,
                            BlockDecodingStrategy::UptoBytes(buffer.len()),
                        )
                        .map_err(std::io::Error::other)?;
                }
                let read = self.decoder.read(buffer)?;
                if read > 0 {
                    self.in_frame = Some(given + read as u64);
                    return Ok(read);
                }
                self.end_frame(given)?;
                self.in_frame = None;
            }
            if self.source.is_empty() {
                if self.begun {
                    return Ok(usize::MIN);
                }
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "a zstd stream holds no frame",
                ));
            }
            self.begun = true;
            let header = self.source;
            match self.decoder.reset(&mut self.source) {
                Ok(()) => self.begin_frame(header)?,
                Err(FrameDecoderError::ReadFrameHeaderError(ReadFrameHeaderError::SkipFrame {
                    length,
                    ..
                })) => {
                    self.source = self.source.get(length as usize..).ok_or_else(|| {
                        std::io::Error::new(
                            std::io::ErrorKind::UnexpectedEof,
                            "a zstd skippable frame runs past the end",
                        )
                    })?;
                }
                Err(error) => return Err(std::io::Error::other(error)),
            }
        }
    }
}

/// Applies one tar to the files; with `whiteouts` as an OCI layer, whose `.wh.` names remove what lower layers hold.
fn read_layer(
    reader: impl Read,
    name: &Path,
    whiteouts: bool,
    files: &mut BTreeMap<Vec<u8>, Entry>,
) -> Result<(), Error> {
    let fail = |what: String| Error::Image(format!("{}: {what}", name.display()));
    let mut archive = tar::Archive::new(reader);
    let mut added: Vec<(Vec<u8>, Entry)> = Vec::new();
    let mut removed: Vec<Vec<u8>> = Vec::new();
    let mut emptied: Vec<Vec<u8>> = Vec::new();
    for entry in archive.entries().map_err(|error| fail(error.to_string()))? {
        let mut entry = entry.map_err(|error| fail(error.to_string()))?;
        let header = entry.header().clone();
        let path = clean(&entry.path_bytes()).ok_or_else(|| {
            fail(format!(
                "entry {} leaves the root",
                String::from_utf8_lossy(&entry.path_bytes())
            ))
        })?;
        if let Some(extensions) = entry.pax_extensions().map_err(|e| fail(e.to_string()))? {
            for extension in extensions {
                let extension = extension.map_err(|e| fail(e.to_string()))?;
                if extension.key_bytes().starts_with(XATTR_PAX_PREFIX) {
                    return Err(fail(format!(
                        "/{} has extended attributes, which the kernel's files do not take yet",
                        String::from_utf8_lossy(&path)
                    )));
                }
            }
        }
        let (parent, base) = match path.iter().rposition(|byte| *byte == SEPARATOR_BYTE) {
            Some(at) => (&path[..at], &path[at + 1..]),
            None => (&path[..0], &path[..]),
        };
        if whiteouts && let Some(hidden) = base.strip_prefix(WHITEOUT_PREFIX) {
            if base == OPAQUE_WHITEOUT {
                emptied.push(parent.to_vec());
            } else {
                removed.push(join(parent, hidden));
            }
            continue;
        }
        let link = || -> Result<Vec<u8>, Error> {
            Ok(header
                .link_name_bytes()
                .ok_or_else(|| fail(LINK_WITHOUT_TARGET.to_string()))?
                .into_owned())
        };
        let device =
            |number: std::io::Result<Option<u32>>| number.ok().flatten().unwrap_or(u32::MIN);
        let kind = match header.entry_type() {
            tar::EntryType::Directory => Kind::Directory,
            tar::EntryType::Regular | tar::EntryType::Continuous => {
                let mut bytes = Vec::new();
                entry
                    .read_to_end(&mut bytes)
                    .map_err(|error| fail(error.to_string()))?;
                Kind::Regular(bytes)
            }
            tar::EntryType::Symlink => Kind::Symlink(link()?),
            tar::EntryType::Link => {
                Kind::HardLink(clean(&link()?).ok_or_else(|| fail(LINK_OUT_OF_ROOT.into()))?)
            }
            tar::EntryType::Char => Kind::CharacterDevice {
                major: device(header.device_major()),
                minor: device(header.device_minor()),
            },
            tar::EntryType::Block => Kind::BlockDevice {
                major: device(header.device_major()),
                minor: device(header.device_minor()),
            },
            tar::EntryType::Fifo => Kind::Fifo,
            tar::EntryType::XGlobalHeader => continue,
            other => {
                return Err(fail(format!(
                    "entry /{} is of a kind this runtime does not unpack ({other:?})",
                    String::from_utf8_lossy(&path)
                )));
            }
        };
        let number = |value: std::io::Result<u64>| value.map_err(|error| fail(error.to_string()));
        let attributes = Attributes {
            mode: header.mode().map_err(|error| fail(error.to_string()))? & PERMISSION_BITS,
            uid: number(header.uid())? as u32,
            gid: number(header.gid())? as u32,
            mtime_seconds: number(header.mtime())? as i64,
        };
        added.push((path, Entry { attributes, kind }));
    }
    for directory in emptied {
        remove_below(files, &directory);
    }
    for path in removed {
        remove_below(files, &path);
        files.remove(&path);
    }
    for (path, entry) in added {
        // A file put where a directory was replaces the directory and all in it.
        if !matches!(entry.kind, Kind::Directory) {
            remove_below(files, &path);
        }
        files.insert(path, entry);
    }
    Ok(())
}

/// Removes everything under the directory at `path`.
fn remove_below(files: &mut BTreeMap<Vec<u8>, Entry>, path: &[u8]) {
    if path.is_empty() {
        files.retain(|key, _| key.is_empty());
        return;
    }
    // Paths under it run from "path/" to just before "path0", '0' following '/'.
    let first = [path, &[SEPARATOR_BYTE]].concat();
    let end = [path, &[SEPARATOR_BYTE + 1]].concat();
    let below: Vec<Vec<u8>> = files
        .range(first..end)
        .map(|(key, _)| key.clone())
        .collect();
    for key in below {
        files.remove(&key);
    }
}

fn join(directory: &[u8], name: &[u8]) -> Vec<u8> {
    if directory.is_empty() {
        name.to_vec()
    } else {
        [directory, &[SEPARATOR_BYTE], name].concat()
    }
}

/// Returns a path from the root with no leading slash, `.` part or trailing slash; none when `..` would leave the root.
fn clean(path: &[u8]) -> Option<Vec<u8>> {
    let mut parts: Vec<&[u8]> = Vec::new();
    for part in split(path) {
        if part == PARENT_PART {
            parts.pop()?;
        } else if part != CURRENT_PART {
            parts.push(part);
        }
    }
    Some(parts.join(&SEPARATOR_BYTE))
}

/// Returns the image's environment with the runtime's defaults (such as `PATH`) for the names it lacks, and no other.
fn with_defaults(defaults: Vec<String>, image: Vec<String>) -> Vec<String> {
    let name = |variable: &str| variable.split(ASSIGNMENT).next().unwrap_or("").to_string();
    let named: Vec<String> = image.iter().map(|v| name(v)).collect();
    defaults
        .into_iter()
        .filter(|default| !named.contains(&name(default)))
        .chain(image)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use wcontainers_kernel_linux::linux::general::S_IWGRP;

    /// Returns a tar of (path, contents) pairs: a path ending in `/` is a directory, contents starting with `->` a link.
    fn layer(entries: &[(&str, &str)]) -> Vec<u8> {
        let mut builder = tar::Builder::new(Vec::new());
        for (path, contents) in entries {
            let mut header = tar::Header::new_gnu();
            header.set_uid(0);
            header.set_gid(0);
            header.set_mtime(0);
            if path.ends_with('/') {
                header.set_entry_type(tar::EntryType::Directory);
                header.set_mode(DIRECTORY_MODE);
                header.set_size(0);
                builder
                    .append_data(&mut header, path, std::io::empty())
                    .unwrap();
            } else if let Some(target) = contents.strip_prefix("->") {
                header.set_entry_type(tar::EntryType::Symlink);
                header.set_mode(0o777);
                header.set_size(0);
                builder.append_link(&mut header, path, target).unwrap();
            } else {
                header.set_entry_type(tar::EntryType::Regular);
                header.set_mode(DIRECTORY_MODE);
                header.set_size(contents.len() as u64);
                builder
                    .append_data(&mut header, path, contents.as_bytes())
                    .unwrap();
            }
        }
        builder.into_inner().unwrap()
    }

    fn image(layers: &[&[(&str, &str)]], config: Option<serde_json::Value>) -> Image {
        let mut files = BTreeMap::new();
        for entries in layers {
            read_layer(
                layer(entries).as_slice(),
                Path::new("layer"),
                true,
                &mut files,
            )
            .unwrap();
        }
        let configuration = config.map(|config| {
            serde_json::from_value(serde_json::json!({
                "architecture": crate::GUEST_ARCHITECTURE, "os": Os::Linux,
                "rootfs": oci_spec::image::RootFs::default(),
                "config": config,
            }))
            .unwrap()
        });
        Image {
            files,
            configuration,
        }
    }

    fn paths(image: &Image) -> Vec<String> {
        image
            .files
            .keys()
            .map(|key| String::from_utf8_lossy(key).into_owned())
            .collect()
    }

    #[test]
    fn whiteouts_remove_what_lower_layers_hold() {
        let image = image(
            &[
                &[
                    ("a/", ""),
                    ("a/one", "1"),
                    ("a/two", "2"),
                    ("b/", ""),
                    ("b/three", "3"),
                    ("c", "4"),
                ],
                &[
                    ("a/.wh.one", ""),
                    ("b/.wh..wh..opq", ""),
                    ("b/five", "5"),
                    ("c/", ""),
                ],
            ],
            None,
        );
        assert_eq!(paths(&image), ["a", "a/two", "b", "b/five", "c"]);
        assert!(matches!(image.files[&b"c".to_vec()].kind, Kind::Directory));
    }

    #[test]
    fn programs_are_found_through_path_and_links() {
        let image = image(
            &[&[
                ("bin/", ""),
                ("bin/busybox", "\x7fELF"),
                ("bin/sh", "->busybox"),
                ("usr/", ""),
                ("usr/bin", "->../bin"),
                ("tmp/", ""),
                ("tmp/program", "\x7fELF"),
            ]],
            None,
        );
        let env = ["PATH=/usr/local/bin:/usr/bin".to_string()];
        let found = |name, cwd| image.program(name, &env, cwd);
        assert_eq!(found("sh", "/").as_deref(), Some("/usr/bin/sh"));
        // Never from the working directory, which holds it.
        assert_eq!(found("program", "/tmp"), None);
        assert_eq!(
            found("./program", "/tmp").as_deref(),
            Some("/tmp/./program")
        );
    }

    #[test]
    fn the_image_configuration_is_converted() {
        let image = image(
            &[&[
                ("etc/", ""),
                (
                    PASSWD,
                    "root:x:0:0::/root:/bin/sh\nweb:x:100:101::/:/bin/sh\n",
                ),
                (GROUP, "root:x:0:\nweb:x:101:\nlog:x:4:web,root\n"),
            ]],
            Some(serde_json::json!({
                "User": "web", "Entrypoint": ["/serve"], "Cmd": ["--port", "80"],
                "Env": ["A=1"], "WorkingDir": "/srv", "StopSignal": "SIGQUIT",
                "Labels": {"name": "web"},
            })),
        );
        let spec = image.spec().unwrap();
        let process = spec.process().as_ref().unwrap();
        assert_eq!(
            process.args().as_ref().unwrap(),
            &["/serve", "--port", "80"]
        );
        assert_eq!(
            process.env().as_ref().unwrap(),
            &[crate::spec::DEFAULT_PATH, "A=1"]
        );
        assert_eq!(process.cwd(), Path::new("/srv"));
        assert_eq!((process.user().uid(), process.user().gid()), (100, 101));
        assert_eq!(
            process.user().additional_gids().as_ref().unwrap(),
            &[101, 4]
        );
        let annotations = spec.annotations().as_ref().unwrap();
        assert_eq!(annotations["name"], "web");
        assert_eq!(annotations[annotation::STOP_SIGNAL], "SIGQUIT");
    }

    #[test]
    fn implicit_annotations_give_way_to_labels() {
        let mut image = image(
            &[&[]],
            Some(serde_json::json!({
                "Env": ["PATH=/bin"], "StopSignal": "SIGQUIT",
                "Labels": {(annotation::STOP_SIGNAL): "SIGINT", (annotation::OS): "label"},
            })),
        );
        let configuration = image.configuration.as_mut().unwrap();
        configuration.set_variant(Some("v8".to_string()));
        configuration.set_os_version(Some("6.1".to_string()));
        configuration.set_os_features(Some(vec!["a".to_string(), "b".to_string()]));
        configuration.set_author(Some("someone".to_string()));
        configuration.set_created(Some("2026-11-18T00:00:00Z".to_string()));
        let spec = image.spec().unwrap();
        let annotations = spec.annotations().as_ref().unwrap();
        let expected = [
            (annotation::OS, "label"),
            (annotation::ARCHITECTURE, crate::GUEST_ARCHITECTURE),
            (annotation::VARIANT, "v8"),
            (annotation::OS_VERSION, "6.1"),
            (annotation::OS_FEATURES, "a,b"),
            (annotation::AUTHOR, "someone"),
            (ANNOTATION_CREATED, "2026-11-18T00:00:00Z"),
            (annotation::STOP_SIGNAL, "SIGINT"),
        ];
        for (name, value) in expected {
            assert_eq!(annotations[name], value);
        }
        assert_eq!(annotations.len(), expected.len());
        let process = spec.process().as_ref().unwrap();
        assert_eq!(process.env().as_ref().unwrap(), &["PATH=/bin"]);
    }

    /// Checks that with no `User`, as Docker, the user is root in its own group, then those that list it in file order.
    #[test]
    fn an_image_without_a_user_runs_as_root_in_its_groups() {
        let files: &[(&str, &str)] = &[
            ("etc/", ""),
            (
                PASSWD,
                "root:x:0:0::/root:/bin/sh
",
            ),
            (
                GROUP,
                "root:x:0:root
wheel:x:10:root
bin:x:1:root,bin
web:x:2:
",
            ),
        ];
        for image in [
            image(&[files], Some(serde_json::json!({}))),
            image(&[files], None),
        ] {
            let spec = image.spec().unwrap();
            let user = spec.process().as_ref().unwrap().user().clone();
            assert_eq!((user.uid(), user.gid()), (0, 0));
            assert_eq!(user.additional_gids().as_ref().unwrap(), &[0, 0, 10, 1]);
            assert_eq!(image.home(0), "/root");
            assert_eq!(image.home(5), "/");
        }
    }

    /// Returns a layer of every kind of entry the layers hold, with the whiteouts of one above it.
    fn layers_of_every_kind() -> [Vec<u8>; 2] {
        let mut builder = tar::Builder::new(Vec::new());
        let mut append = |path: &str, kind, mode, link: Option<&str>, contents: &[u8]| {
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(kind);
            header.set_mode(mode);
            header.set_uid(7);
            header.set_gid(9);
            header.set_mtime(1_700_000_000);
            header.set_size(contents.len() as u64);
            if kind == tar::EntryType::Char || kind == tar::EntryType::Block {
                header.set_device_major(1).unwrap();
                header.set_device_minor(3).unwrap();
            }
            match link {
                Some(target) => builder.append_link(&mut header, path, target).unwrap(),
                None => builder.append_data(&mut header, path, contents).unwrap(),
            }
        };
        let big: Vec<u8> = (0..300_000u32)
            .map(|n| (n.wrapping_mul(n) >> 7) as u8)
            .collect();
        append("d/", tar::EntryType::Directory, DIRECTORY_MODE, None, b"");
        append(
            "d/file",
            tar::EntryType::Regular,
            S_ISUID | FILE_MODE,
            None,
            b"text\n",
        );
        append(
            "d/big",
            tar::EntryType::Regular,
            S_IRUSR | S_IWUSR,
            None,
            &big,
        );
        append("d/gone", tar::EntryType::Regular, FILE_MODE, None, b"x");
        append("d/link", tar::EntryType::Symlink, 0o777, Some("file"), b"");
        append(
            "d/hard",
            tar::EntryType::Link,
            FILE_MODE,
            Some("d/file"),
            b"",
        );
        append(
            "e/",
            tar::EntryType::Directory,
            S_ISVTX | S_IRWXU | S_IRWXG | S_IRWXO,
            None,
            b"",
        );
        append("e/inside", tar::EntryType::Regular, FILE_MODE, None, b"y");
        append("null", tar::EntryType::Char, 0o666, None, b"");
        append(
            "block",
            tar::EntryType::Block,
            S_IRUSR | S_IWUSR | S_IRGRP | S_IWGRP,
            None,
            b"",
        );
        append("fifo", tar::EntryType::Fifo, FILE_MODE, None, b"");
        let lower = builder.into_inner().unwrap();
        let upper = layer(&[("d/.wh.gone", ""), ("e/.wh..wh..opq", ""), ("e/new", "z")]);
        [lower, upper]
    }

    /// Returns a descriptor of `bytes` as a blob of `media_type`.
    fn descriptor(media_type: MediaType, bytes: &[u8]) -> Descriptor {
        let digest = format!("sha256:{:x}", Sha256::digest(bytes));
        Descriptor::new(
            media_type,
            bytes.len() as u64,
            digest.parse::<oci_spec::image::Digest>().unwrap(),
        )
    }

    /// Returns the files the layers give, each compressed by `compress` as a layer of `media_type`.
    fn unpacked(
        layers: &[Vec<u8>],
        media_type: MediaType,
        compress: impl Fn(&[u8]) -> Vec<u8>,
    ) -> Result<BTreeMap<Vec<u8>, Entry>, Error> {
        let mut files = BTreeMap::new();
        for layer in layers {
            let bytes = compress(layer);
            let descriptor = descriptor(media_type.clone(), &bytes);
            apply_layer(&descriptor, &bytes, Path::new("layer"), &mut files)?;
        }
        Ok(files)
    }

    fn gzip(bytes: &[u8]) -> Vec<u8> {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        std::io::Write::write_all(&mut encoder, bytes).unwrap();
        encoder.finish().unwrap()
    }

    /// Returns `bytes` as libzstd compresses them at `level`, with a checksum and the content size.
    fn libzstd(bytes: &[u8], level: i32) -> Vec<u8> {
        let mut encoder = zstd::stream::Encoder::new(Vec::new(), level).unwrap();
        encoder.include_checksum(true).unwrap();
        encoder
            .set_pledged_src_size(Some(bytes.len() as u64))
            .unwrap();
        std::io::Write::write_all(&mut encoder, bytes).unwrap();
        encoder.finish().unwrap()
    }

    /// Returns a skippable frame of `length` bytes, with the `n`th of its magic numbers (RFC 8878 3.1.2).
    fn skippable(n: u32, length: u32) -> Vec<u8> {
        let mut frame = (0x184D_2A50 + n).to_le_bytes().to_vec();
        frame.extend(length.to_le_bytes());
        frame.extend((0..length).map(|byte| byte as u8));
        frame
    }

    /// Returns what our reader gives of `stream`, read in pieces of `piece` bytes.
    fn ours(stream: &[u8], piece: usize) -> std::io::Result<Vec<u8>> {
        let mut reader = Zstd::new(stream);
        let mut out = Vec::new();
        let mut buffer = vec![0; piece];
        loop {
            let read = reader.read(&mut buffer)?;
            if read == 0 {
                return Ok(out);
            }
            out.extend_from_slice(&buffer[..read]);
        }
    }

    fn the_same_files(one: &BTreeMap<Vec<u8>, Entry>, other: &BTreeMap<Vec<u8>, Entry>) -> bool {
        format!("{one:?}") == format!("{other:?}")
    }

    #[test]
    fn a_zstd_layer_gives_the_files_of_its_gzip_twin() {
        let layers = layers_of_every_kind();
        let plain = unpacked(&layers, MediaType::ImageLayer, <[u8]>::to_vec).unwrap();
        let gzip = unpacked(&layers, MediaType::ImageLayerGzip, gzip).unwrap();
        assert!(the_same_files(&plain, &gzip));
        for media_type in [
            MediaType::ImageLayerZstd,
            MediaType::ImageLayerNonDistributableZstd,
        ] {
            let zstd = unpacked(&layers, media_type, |bytes| libzstd(bytes, 3)).unwrap();
            assert!(the_same_files(&gzip, &zstd));
        }
        let names: Vec<_> = gzip
            .keys()
            .map(|key| String::from_utf8_lossy(key))
            .collect();
        assert_eq!(
            names,
            [
                "block", "d", "d/big", "d/file", "d/hard", "d/link", "e", "e/new", "fifo", "null"
            ]
            .map(str::to_string)
        );
    }

    /// Checks that seeded random streams of several frames, skippable ones among them, decode as libzstd decodes them.
    ///
    /// Frames come with and without checksums and sizes; `ZSTD_SEED` (0 if unset) and `ZSTD_CASES` (100 if unset)
    /// choose the streams.
    #[test]
    fn zstd_streams_decode_as_libzstd_decodes_them() {
        let seed: u64 = std::env::var("ZSTD_SEED").map_or(0, |seed| seed.parse().unwrap());
        let cases: u64 = std::env::var("ZSTD_CASES").map_or(100, |cases| cases.parse().unwrap());
        let mut state = seed;
        // splitmix64
        let mut next = move |below: u64| {
            state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = state;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            (z ^ (z >> 31)) % below
        };
        for case in 0..cases {
            let (mut stream, mut expected) = (Vec::new(), Vec::new());
            for _ in 0..1 + next(4) {
                if next(3) == 0 {
                    stream.extend(skippable(next(16) as u32, next(64) as u32));
                }
                let alphabet = 1 + next(255);
                let data: Vec<u8> = (0..next(400_000)).map(|_| next(alphabet) as u8).collect();
                let mut encoder =
                    zstd::stream::Encoder::new(Vec::new(), 1 + next(12) as i32).unwrap();
                encoder.include_checksum(next(2) == 0).unwrap();
                if next(2) == 0 {
                    encoder
                        .set_pledged_src_size(Some(data.len() as u64))
                        .unwrap();
                } else {
                    encoder.include_contentsize(false).unwrap();
                }
                std::io::Write::write_all(&mut encoder, &data).unwrap();
                stream.extend(encoder.finish().unwrap());
                expected.extend(data);
            }
            // zstd:chunked layers end with skippable frames.
            for _ in 0..next(3) {
                stream.extend(skippable(next(16) as u32, next(4000) as u32));
            }
            let piece = 1 + next(99_991) as usize;
            assert_eq!(
                zstd::stream::decode_all(stream.as_slice()).unwrap(),
                expected
            );
            assert!(
                ours(&stream, piece).unwrap() == expected,
                "case {case} of seed {seed}"
            );
        }
        eprintln!("{cases} cases of seed {seed}");
    }

    /// Checks that streams libzstd refuses, cut, changed, sized wrongly or with a reserved bit set, are refused too.
    #[test]
    fn damaged_zstd_streams_are_refused_as_libzstd_refuses_them() {
        let data: Vec<u8> = (0..99_991u32)
            .map(|n| ((n % 251) ^ (n >> 9)) as u8)
            .collect();
        let good = libzstd(&data, 3);
        let mut damaged = Vec::new();
        // A changed byte inside a block, and a changed checksum.
        for at in [good.len() / 2, good.len() - 1] {
            let mut stream = good.clone();
            stream[at] ^= 0x40;
            damaged.push(stream);
        }
        damaged.push(good[..good.len() - 7].to_vec());
        damaged.push([good.as_slice(), b"after"].concat());
        let mut skipping_past_the_end = skippable(0, 8);
        skipping_past_the_end.truncate(12);
        damaged.push(skipping_past_the_end);
        // A declared size one more than the contents: one byte after the descriptor of a frame
        // whose descriptor says a single segment with a one-byte size.
        let small = libzstd(b"small", 3);
        let header = small[4];
        assert_eq!((header >> 6, (header >> 5) & 1), (0, 1));
        let mut wrong_size = small.clone();
        wrong_size[5] += 1;
        damaged.push(wrong_size);
        // A frame that sets the reserved bit of its header descriptor.
        let mut reserved = good.clone();
        reserved[ZSTD_DESCRIPTOR_AT] |= ZSTD_RESERVED;
        damaged.push(reserved);
        // A frame with a four-byte declared size, a window of 1 KiB and one raw last block of five
        // bytes (RFC 8878 3.1.1.1 and 3.1.1.2): it decodes when it declares five, not when zero.
        let raw_frame = |declared: u32| {
            let mut frame = small[..ZSTD_DESCRIPTOR_AT].to_vec();
            frame.extend([0b1000_0000, 0]);
            frame.extend(declared.to_le_bytes());
            frame.extend([1 | (5 << 3), 0, 0]);
            frame.extend(b"hello");
            frame
        };
        let declared_five = raw_frame(5);
        assert_eq!(
            zstd::stream::decode_all(declared_five.as_slice()).unwrap(),
            b"hello"
        );
        assert_eq!(ours(&declared_five, 4093).unwrap(), b"hello");
        damaged.push(raw_frame(0));
        // No frame at all.
        damaged.push(Vec::new());
        // A frame that needs a window above libzstd's default limit, 128 MiB.
        let mut encoder = zstd::stream::Encoder::new(Vec::new(), 3).unwrap();
        encoder.include_contentsize(false).unwrap();
        encoder.window_log(28).unwrap();
        std::io::Write::write_all(&mut encoder, &data).unwrap();
        damaged.push(encoder.finish().unwrap());
        for (n, stream) in damaged.iter().enumerate() {
            assert!(zstd::stream::decode_all(stream.as_slice()).is_err(), "{n}");
            assert!(ours(stream, 4093).is_err(), "{n}");
        }
        // And a damaged layer is refused, even after the tar's end.
        let layers = layers_of_every_kind();
        let mut layer = libzstd(&layers[0], 3);
        let last = layer.len() - 1;
        layer[last] ^= 1;
        let descriptor = descriptor(MediaType::ImageLayerZstd, &layer);
        let mut files = BTreeMap::new();
        assert!(matches!(
            apply_layer(&descriptor, &layer, Path::new("layer"), &mut files),
            Err(Error::Image(_))
        ));
    }

    #[test]
    fn volumes_are_refused_by_name() {
        let image = image(&[&[]], Some(serde_json::json!({"Volumes": {"/data": {}}})));
        match image.spec() {
            Err(Error::Refused { field, .. }) => assert_eq!(field, CONFIG_VOLUMES),
            other => panic!("{:?}", other.map(|_| ())),
        }
    }
}
