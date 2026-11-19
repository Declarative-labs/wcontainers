//! A container's root files: a plain tar of files, or an image in the OCI image layout, whose layers
//! are applied in order with their whiteouts and whose configuration becomes the OCI runtime
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
use sha2::{Digest as _, Sha256};
use wcontainers_kernel::linux::general::{
    S_IRGRP, S_IROTH, S_IRUSR, S_IRWXG, S_IRWXO, S_IRWXU, S_ISGID, S_ISUID, S_ISVTX, S_IWUSR,
    S_IXGRP, S_IXOTH, S_IXUSR,
};
use wcontainers_kernel::{Attributes, Kernel, NewFile};

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
    /// By path from the root without a leading slash ("" is the root itself), so that a directory
    /// comes before what is in it.
    files: BTreeMap<Vec<u8>, Entry>,
    configuration: Option<ImageConfiguration>,
}

/// Symbolic links followed in one lookup before it fails, as Linux's `MAXSYMLINKS`.
const MOST_LINKS: usize = 40;

/// The permission bits of a mode, the rest being its kind.
const PERMISSION_BITS: u32 = S_ISUID | S_ISGID | S_ISVTX | S_IRWXU | S_IRWXG | S_IRWXO;
/// Execute permission for anyone.
const EXECUTABLE: u32 = S_IXUSR | S_IXGRP | S_IXOTH;
/// `rwxr-xr-x` and `rw-r--r--`, the modes Docker gives the directory and files it writes.
const DIRECTORY_MODE: u32 = S_IRWXU | S_IRGRP | S_IXGRP | S_IROTH | S_IXOTH;
const FILE_MODE: u32 = S_IRUSR | S_IWUSR | S_IRGRP | S_IROTH;

/// The OCI image layout's marker file, its version, its index and its blobs' directory
/// (image-layout.md).
const LAYOUT_FILE: &str = "oci-layout";
const LAYOUT_VERSION: &str = "1.0.0";
const INDEX_FILE: &str = "index.json";
const BLOBS_DIRECTORY: &str = "blobs";

/// Docker's media type of a gzip-compressed layer, which `docker save` writes.
const DOCKER_LAYER_GZIP: &str = "application/vnd.docker.image.rootfs.diff.tar.gzip";

/// The start of a whiteout's name, and the name of the whiteout that empties its directory
/// (layer.md).
const WHITEOUT_PREFIX: &[u8] = b".wh.";
const OPAQUE_WHITEOUT: &[u8] = b".wh..wh..opq";

/// The start of a PAX header key holding an extended attribute.
const XATTR_PAX_PREFIX: &[u8] = b"SCHILY.xattr.";

/// The annotations conversion.md makes of an image configuration's fields.
mod annotation {
    pub const OS: &str = "org.opencontainers.image.os";
    pub const ARCHITECTURE: &str = "org.opencontainers.image.architecture";
    pub const VARIANT: &str = "org.opencontainers.image.variant";
    pub const OS_VERSION: &str = "org.opencontainers.image.os.version";
    pub const OS_FEATURES: &str = "org.opencontainers.image.os.features";
    pub const AUTHOR: &str = "org.opencontainers.image.author";
    pub const STOP_SIGNAL: &str = "org.opencontainers.image.stopSignal";
    pub const EXPOSED_PORTS: &str = "org.opencontainers.image.exposedPorts";
}

/// What joins the items of a list in an annotation (conversion.md).
const LIST_SEPARATOR: &str = ",";

/// The image configuration's fields this runtime refuses values of, as the refusals name them.
const CONFIG_VOLUMES: &str = "config.Volumes";
const CONFIG_USER: &str = "config.User";

/// Fields of a line of `/etc/passwd` (name, password, uid, gid, comment, home, shell) and of
/// `/etc/group` (name, password, gid, members), by their places (passwd(5), group(5)).
mod field {
    pub const NAME: usize = 0;
    pub const UID: usize = 2;
    pub const GID: usize = 3;
    pub const HOME: usize = 5;
    pub const GROUP_GID: usize = 2;
    pub const GROUP_MEMBERS: usize = 3;
}

impl Image {
    /// A plain tar of the root's files, such as `docker export` writes.
    pub fn tar(path: &Path) -> Result<Self, Error> {
        let mut files = BTreeMap::new();
        read_layer(open(path)?, path, false, &mut files)?;
        Ok(Self {
            files,
            configuration: None,
        })
    }

    /// An image in the OCI image layout (the directory `docker save` writes as a tar): the manifest for
    /// `architecture` (as the image configuration names it, [`crate::Executor::ARCHITECTURE`]) on
    /// Linux, its blobs checked against their digests and its layers applied in order.
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
            let name = blob_path(directory, layer);
            match layer.media_type() {
                MediaType::ImageLayer | MediaType::ImageLayerNonDistributable => {
                    read_layer(bytes.as_slice(), &name, true, &mut files)?;
                }
                MediaType::ImageLayerGzip | MediaType::ImageLayerNonDistributableGzip => {
                    let reader = flate2::read::GzDecoder::new(bytes.as_slice());
                    read_layer(reader, &name, true, &mut files)?;
                }
                MediaType::Other(other) if other == DOCKER_LAYER_GZIP => {
                    let reader = flate2::read::GzDecoder::new(bytes.as_slice());
                    read_layer(reader, &name, true, &mut files)?;
                }
                other => {
                    return Err(Error::Image(format!(
                        "layer {} has media type {other}, which this runtime does not unpack",
                        layer.digest()
                    )));
                }
            }
        }
        Ok(Self {
            files,
            configuration: Some(configuration),
        })
    }

    /// The architecture an OCI image is for; none for a plain tar.
    pub fn architecture(&self) -> Option<String> {
        self.configuration
            .as_ref()
            .map(|configuration| configuration.architecture().to_string())
    }

    /// What to run: for an OCI image, the runtime's base configuration ([`crate::base_spec`]) with the
    /// image configuration converted into it as `conversion.md` says; for a plain tar, the base
    /// configuration, whose `process.args` the caller sets. Where the image names no user, the user
    /// is root with the groups `/etc/group` lists it in, as Docker gives it.
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
            process.set_cwd(PathBuf::from(cwd.unwrap_or_else(|| "/".to_string())));
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

    /// `process.user` for the image configuration's `User` (`user`, `uid`, `user:group`, `uid:gid`
    /// and the mixed forms), names looked up in the root's `/etc/passwd` and `/etc/group`.
    fn user(&self, text: &str) -> Result<User, Error> {
        let passwd = self.table(b"etc/passwd");
        let group = self.table(b"etc/group");
        let (name, group_name) = match text.split_once(':') {
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
                .ok_or_else(|| refused(format!("no user {name} in /etc/passwd")))?,
        };
        let gid = match group_name {
            Some(group_name) => match group_name.parse::<u32>() {
                Ok(gid) => gid,
                Err(_) => group
                    .iter()
                    .find(|fields| fields.get(field::NAME) == Some(&group_name))
                    .and_then(|fields| fields.get(field::GROUP_GID)?.parse().ok())
                    .ok_or_else(|| refused(format!("no group {group_name} in /etc/group")))?,
            },
            None => account
                .and_then(|fields| fields.get(field::GID)?.parse().ok())
                .unwrap_or(0),
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

    /// The user where the image names none: root, in its groups, as Docker gives it.
    fn root(&self) -> User {
        let passwd = self.table(b"etc/passwd");
        let account = passwd
            .iter()
            .find(|fields| fields.get(field::UID).and_then(|f| f.parse().ok()) == Some(0));
        let gid = account
            .and_then(|fields| fields.get(field::GID)?.parse().ok())
            .unwrap_or(0);
        let mut user = User::default();
        user.set_gid(gid);
        let name = account.and_then(|fields| fields.get(field::NAME).copied());
        user.set_additional_gids(Some(
            name.map_or_else(|| vec![gid], |name| self.groups(name, gid)),
        ));
        user
    }

    /// The additional groups of the user `name` whose group is `gid`, as Docker gives them: `gid`,
    /// then each group `/etc/group` lists the user in, in its order.
    fn groups(&self, name: &str, gid: u32) -> Vec<u32> {
        let members = self
            .table(b"etc/group")
            .into_iter()
            .filter(|fields| {
                fields
                    .get(field::GROUP_MEMBERS)
                    .is_some_and(|members| members.split(',').any(|m| m == name))
            })
            .filter_map(|fields| fields.get(field::GROUP_GID)?.parse().ok());
        std::iter::once(gid).chain(members).collect()
    }

    /// The home directory of `uid` in the root's `/etc/passwd`, or `/` as runc has it.
    pub(crate) fn home(&self, uid: u32) -> String {
        self.table(b"etc/passwd")
            .iter()
            .find(|fields| fields.get(field::UID).and_then(|f| f.parse().ok()) == Some(uid))
            .and_then(|fields| fields.get(field::HOME))
            .filter(|home| !home.is_empty())
            .map_or_else(|| "/".to_string(), |home| home.to_string())
    }

    /// The lines of a colon-separated table in the root, such as `/etc/passwd`, split into fields.
    fn table(&self, path: &[u8]) -> Vec<Vec<&str>> {
        match self.lookup(path) {
            Some(Entry {
                kind: Kind::Regular(bytes),
                ..
            }) => std::str::from_utf8(bytes)
                .unwrap_or("")
                .lines()
                .filter(|line| !line.starts_with('#'))
                .map(|line| line.split(':').collect())
                .collect(),
            _ => Vec::new(),
        }
    }

    /// The file at `path` (from the root, with `/` between parts), following symbolic links.
    fn lookup(&self, path: &[u8]) -> Option<&Entry> {
        let mut parts: Vec<Vec<u8>> = split(path).rev().map(<[u8]>::to_vec).collect();
        let mut at: Vec<Vec<u8>> = Vec::new();
        let mut links = 0;
        while let Some(part) = parts.pop() {
            match part.as_slice() {
                b"." => continue,
                b".." => {
                    at.pop();
                    continue;
                }
                _ => {}
            }
            at.push(part);
            let entry = self.files.get(&at.join(&b'/'))?;
            if let Kind::Symlink(target) = &entry.kind {
                links += 1;
                if links > MOST_LINKS {
                    return None;
                }
                at.pop();
                if target.starts_with(b"/") {
                    at.clear();
                }
                parts.extend(split(target).rev().map(<[u8]>::to_vec));
            } else if !parts.is_empty() && !matches!(entry.kind, Kind::Directory) {
                return None;
            }
        }
        self.files.get(&at.join(&b'/'))
    }

    /// The program `process.args[0]` names, as runc finds it: a name with a slash from the working
    /// directory, any other name in the directories of `PATH` in `env`, the first that is a file anyone
    /// may run. None for a name with no slash found in none of them; a path with a slash is the
    /// kernel's to fail as exec does.
    pub(crate) fn program(&self, name: &str, env: &[String], cwd: &str) -> Option<String> {
        if name.contains('/') {
            return Some(if name.starts_with('/') {
                name.to_string()
            } else {
                format!("{}/{name}", cwd.trim_end_matches('/'))
            });
        }
        let path = env
            .iter()
            .rev()
            .find_map(|variable| variable.strip_prefix("PATH="))
            .unwrap_or("");
        path.split(':')
            .filter(|directory| !directory.is_empty())
            .map(|directory| format!("{}/{name}", directory.trim_end_matches('/')))
            .find(|candidate| {
                matches!(
                    self.lookup(candidate.as_bytes()),
                    Some(Entry { attributes, kind: Kind::Regular(_) }) if attributes.mode & EXECUTABLE != 0
                )
            })
    }

    /// Puts the files in a new kernel's root, directories before what they hold and hard links last,
    /// with the files Docker writes in every container in place of the image's own.
    pub(crate) fn add_to(&self, kernel: &mut Kernel, hostname: &str) -> Result<(), Error> {
        let written = docker_files(hostname);
        let etc = match self.files.get(&b"etc"[..]) {
            None => Some((
                b"etc".to_vec(),
                docker_file(DIRECTORY_MODE, Kind::Directory),
            )),
            Some(Entry {
                kind: Kind::Directory,
                ..
            }) => None,
            Some(_) => {
                return Err(Error::Image(
                    "/etc is not a directory, where the host name and name server files go".into(),
                ));
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
                .map_err(|error| {
                    Error::Image(format!(
                        "/{}: the kernel refused it ({error:?})",
                        String::from_utf8_lossy(path)
                    ))
                })?;
        }
        Ok(())
    }
}

/// What Docker writes over its mounts in every container: the host name, the hosts file with the
/// kernel's made-up address for it, and the kernel's name server.
fn docker_files(hostname: &str) -> [(Vec<u8>, Entry); 3] {
    let address = std::net::Ipv4Addr::from(wcontainers_kernel::network::ADDRESS);
    let name_server = std::net::Ipv4Addr::from(wcontainers_kernel::network::NAME_SERVER);
    let (v4, v6) = (std::net::Ipv4Addr::LOCALHOST, std::net::Ipv6Addr::LOCALHOST);
    let hosts = format!(
        "{v4}\tlocalhost\n{v6}\tlocalhost ip6-localhost ip6-loopback\nfe00::\tip6-localnet\n\
         ff00::\tip6-mcastprefix\nff02::1\tip6-allnodes\nff02::2\tip6-allrouters\n\
         {address}\t{hostname}\n"
    );
    let file = |bytes: String| docker_file(FILE_MODE, Kind::Regular(bytes.into_bytes()));
    [
        (b"etc/hostname".to_vec(), file(format!("{hostname}\n"))),
        (b"etc/hosts".to_vec(), file(hosts)),
        (
            b"etc/resolv.conf".to_vec(),
            file(format!("nameserver {name_server}\n")),
        ),
    ]
}

/// An entry owned by root, dated at 0 so that a run repeats.
fn docker_file(mode: u32, kind: Kind) -> Entry {
    Entry {
        attributes: Attributes {
            mode,
            uid: 0,
            gid: 0,
            mtime_seconds: 0,
        },
        kind,
    }
}

/// The parts of a path, without empty ones.
fn split(path: &[u8]) -> impl DoubleEndedIterator<Item = &[u8]> {
    path.split(|byte| *byte == b'/')
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

/// A blob's bytes, checked against its descriptor's size and digest.
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

/// The manifest for linux and `architecture` among an index's descriptors, through nested indexes.
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

/// Applies one tar to the files: with `whiteouts`, as an OCI layer (`.wh.` names remove what lower
/// layers hold, `.wh..wh..opq` empties a directory of it).
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
        let (parent, base) = match path.iter().rposition(|byte| *byte == b'/') {
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
                .ok_or_else(|| fail("a link without a target".to_string()))?
                .into_owned())
        };
        let device = |number: std::io::Result<Option<u32>>| number.ok().flatten().unwrap_or(0);
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
            tar::EntryType::Link => Kind::HardLink(
                clean(&link()?).ok_or_else(|| fail("a link out of the root".into()))?,
            ),
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

/// Everything under the directory at `path`.
fn remove_below(files: &mut BTreeMap<Vec<u8>, Entry>, path: &[u8]) {
    if path.is_empty() {
        files.retain(|key, _| key.is_empty());
        return;
    }
    // Paths under it run from "path/" to just before "path0", '0' following '/'.
    let first = [path, b"/"].concat();
    let end = [path, b"0"].concat();
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
        [directory, b"/", name].concat()
    }
}

/// A path from the root without a leading slash, `.` parts or a trailing slash; none when `..` would
/// leave the root.
fn clean(path: &[u8]) -> Option<Vec<u8>> {
    let mut parts: Vec<&[u8]> = Vec::new();
    for part in split(path) {
        match part {
            b"." => {}
            b".." => {
                parts.pop()?;
            }
            _ => parts.push(part),
        }
    }
    Some(parts.join(&b'/'))
}

/// The image's environment with the runtime's defaults (such as `PATH`) for the names it lacks, so
/// that a default never removes or changes one of the image's variables.
fn with_defaults(defaults: Vec<String>, image: Vec<String>) -> Vec<String> {
    let name = |variable: &str| variable.split('=').next().unwrap_or("").to_string();
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

    /// A tar of (path, contents) pairs: a path ending in `/` is a directory, contents starting with
    /// `->` a symbolic link.
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
                "architecture": "arm64", "os": "linux",
                "rootfs": {"type": "layers", "diff_ids": []},
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
                ("tmp/run", "\x7fELF"),
            ]],
            None,
        );
        let env = ["PATH=/usr/local/bin:/usr/bin".to_string()];
        let found = |name, cwd| image.program(name, &env, cwd);
        assert_eq!(found("sh", "/").as_deref(), Some("/usr/bin/sh"));
        // Never from the working directory, which holds it.
        assert_eq!(found("run", "/tmp"), None);
        assert_eq!(found("./run", "/tmp").as_deref(), Some("/tmp/./run"));
    }

    #[test]
    fn the_image_configuration_is_converted() {
        let image = image(
            &[&[
                ("etc/", ""),
                (
                    "etc/passwd",
                    "root:x:0:0::/root:/bin/sh\nweb:x:100:101::/:/bin/sh\n",
                ),
                ("etc/group", "root:x:0:\nweb:x:101:\nlog:x:4:web,root\n"),
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
            &[
                "PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
                "A=1"
            ]
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
            (annotation::ARCHITECTURE, "arm64"),
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

    /// With no `User`, as Docker: root, in its own group and then the groups that list it, in file
    /// order.
    #[test]
    fn an_image_without_a_user_runs_as_root_in_its_groups() {
        let files: &[(&str, &str)] = &[
            ("etc/", ""),
            (
                "etc/passwd",
                "root:x:0:0::/root:/bin/sh
",
            ),
            (
                "etc/group",
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

    #[test]
    fn volumes_are_refused_by_name() {
        let image = image(&[&[]], Some(serde_json::json!({"Volumes": {"/data": {}}})));
        match image.spec() {
            Err(Error::Refused { field, .. }) => assert_eq!(field, CONFIG_VOLUMES),
            other => panic!("{:?}", other.map(|_| ())),
        }
    }
}
