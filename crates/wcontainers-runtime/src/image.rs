//! A container's root files: a plain tar of files, or an image in the OCI image layout, whose layers
//! are applied in order with their whiteouts and whose configuration becomes the OCI runtime
//! configuration by the image specification's conversion (`conversion.md`).

use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use oci_spec::image::{Descriptor, ImageConfiguration, ImageIndex, ImageManifest, MediaType};
use oci_spec::runtime::{Spec, User};
use sha2::{Digest as _, Sha256};
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
    /// `architecture` (as the image configuration names it, [`crate::BackEnd::ARCHITECTURE`]) on
    /// Linux, its blobs checked against their digests and its layers applied in order.
    pub fn oci(directory: &Path, architecture: &str) -> Result<Self, Error> {
        let layout = directory.join("oci-layout");
        let header: serde_json::Value = serde_json::from_reader(open(&layout)?)
            .map_err(|error| Error::Image(format!("{}: {error}", layout.display())))?;
        if header["imageLayoutVersion"].as_str() != Some("1.0.0") {
            return Err(Error::Image(format!(
                "{}: image layout version {} is not 1.0.0",
                layout.display(),
                header["imageLayoutVersion"]
            )));
        }
        let index_path = directory.join("index.json");
        let index = ImageIndex::from_reader(open(&index_path)?)
            .map_err(|error| Error::Image(format!("{}: {error}", index_path.display())))?;
        let manifest = find_manifest(directory, index.manifests(), architecture)?;
        let config_bytes = blob(directory, manifest.config())?;
        let configuration = ImageConfiguration::from_reader(config_bytes.as_slice())
            .map_err(|error| Error::Image(format!("the image configuration: {error}")))?;
        if configuration.os().to_string() != "linux"
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
                MediaType::Other(other)
                    if other == "application/vnd.docker.image.rootfs.diff.tar.gzip" =>
                {
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
    /// configuration alone, whose `process.args` the caller sets.
    pub fn spec(&self) -> Result<Spec, Error> {
        let mut spec = crate::base_spec();
        let Some(configuration) = &self.configuration else {
            return Ok(spec);
        };
        let Some(config) = configuration.config() else {
            return Ok(spec);
        };
        let mut process = spec.process().clone().unwrap_or_default();
        let mut args: Vec<String> = config.entrypoint().clone().unwrap_or_default();
        args.extend(config.cmd().clone().unwrap_or_default());
        process.set_args(Some(args));
        process.set_env(config.env().clone());
        let cwd = config.working_dir().clone().filter(|cwd| !cwd.is_empty());
        process.set_cwd(PathBuf::from(cwd.unwrap_or_else(|| "/".to_string())));
        if let Some(user) = config.user().as_deref().filter(|user| !user.is_empty()) {
            process.set_user(self.user(user)?);
        }
        spec.set_process(Some(process));
        let mut annotations: HashMap<String, String> = config.labels().clone().unwrap_or_default();
        if let Some(signal) = config.stop_signal() {
            annotations.insert(
                "org.opencontainers.image.stopSignal".to_string(),
                signal.clone(),
            );
        }
        if let Some(ports) = config.exposed_ports().as_ref().filter(|p| !p.is_empty()) {
            annotations.insert(
                "org.opencontainers.image.exposedPorts".to_string(),
                ports.join(","),
            );
        }
        if !annotations.is_empty() {
            spec.set_annotations(Some(annotations));
        }
        if let Some(volumes) = config.volumes().as_ref().filter(|v| !v.is_empty()) {
            return Err(Error::Refused {
                field: "config.Volumes".to_string(),
                reason: format!(
                    "{volumes:?}: a volume is a mount of a directory from outside, which needs the \
                     shipped outside's file backend"
                ),
            });
        }
        Ok(spec)
    }

    /// `process.user` for the image configuration's `User` (`user`, `uid`, `user:group`, `uid:gid`
    /// and the mixed forms), names looked up in the root's `/etc/passwd` and `/etc/group`, with the
    /// groups that list the user as additional groups.
    fn user(&self, text: &str) -> Result<User, Error> {
        let passwd = self.table(b"etc/passwd");
        let group = self.table(b"etc/group");
        let (name, group_name) = match text.split_once(':') {
            Some((name, group)) => (name, Some(group)),
            None => (text, None),
        };
        let refused = |reason: String| Error::Refused {
            field: "config.User".to_string(),
            reason,
        };
        let account = passwd.iter().find(|fields| {
            fields.first() == Some(&name)
                || name
                    .parse::<u32>()
                    .is_ok_and(|uid| fields.get(2).and_then(|f| f.parse().ok()) == Some(uid))
        });
        let uid = match name.parse::<u32>() {
            Ok(uid) => uid,
            Err(_) => account
                .and_then(|fields| fields.get(2)?.parse().ok())
                .ok_or_else(|| refused(format!("no user {name} in /etc/passwd")))?,
        };
        let gid = match group_name {
            Some(group_name) => match group_name.parse::<u32>() {
                Ok(gid) => gid,
                Err(_) => group
                    .iter()
                    .find(|fields| fields.first() == Some(&group_name))
                    .and_then(|fields| fields.get(2)?.parse().ok())
                    .ok_or_else(|| refused(format!("no group {group_name} in /etc/group")))?,
            },
            None => account
                .and_then(|fields| fields.get(3)?.parse().ok())
                .unwrap_or(0),
        };
        let user_name = account.and_then(|fields| fields.first().copied());
        let mut additional: Vec<u32> = match (group_name, user_name) {
            (None, Some(user_name)) => group
                .iter()
                .filter(|fields| {
                    fields
                        .get(3)
                        .is_some_and(|members| members.split(',').any(|m| m == user_name))
                })
                .filter_map(|fields| fields.get(2)?.parse().ok())
                .filter(|&g| g != gid)
                .collect(),
            _ => Vec::new(),
        };
        additional.sort_unstable();
        additional.dedup();
        let mut user = User::default();
        user.set_uid(uid);
        user.set_gid(gid);
        user.set_additional_gids((!additional.is_empty()).then_some(additional));
        Ok(user)
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
    /// may run. A name not found is handed on as it is, for the kernel to fail as exec does.
    pub(crate) fn program(&self, name: &str, env: &[String], cwd: &str) -> String {
        if name.contains('/') {
            return if name.starts_with('/') {
                name.to_string()
            } else {
                format!("{}/{name}", cwd.trim_end_matches('/'))
            };
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
                    Some(Entry { attributes, kind: Kind::Regular(_) }) if attributes.mode & 0o111 != 0
                )
            })
            .unwrap_or_else(|| name.to_string())
    }

    /// Puts the files in a new kernel's root, directories before what they hold and hard links last.
    pub(crate) fn add_to(&self, kernel: &mut Kernel) -> Result<(), Error> {
        let hard_links = self
            .files
            .iter()
            .filter(|(_, entry)| matches!(entry.kind, Kind::HardLink(_)));
        let others = self
            .files
            .iter()
            .filter(|(_, entry)| !matches!(entry.kind, Kind::HardLink(_)));
        for (path, entry) in others.chain(hard_links) {
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
        .join("blobs")
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
    if digest.algorithm().to_string() != "sha256" {
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
            platform.os().to_string() == "linux"
                && platform.architecture().to_string() == architecture
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
                if extension.key_bytes().starts_with(b"SCHILY.xattr.") {
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
        if whiteouts && let Some(hidden) = base.strip_prefix(b".wh.") {
            if hidden == b".wh..opq" {
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
            mode: header.mode().map_err(|error| fail(error.to_string()))? & 0o7777,
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
