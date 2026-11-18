//! The cache file (plan section 8): entries keyed by 32-byte BLAKE3 hashes of their content, appended
//! to one file that loads into an index, merges by union and survives a cut-off last entry.
//!
//! The file is a 16-byte header (`WCGPUCACHE`, the format version as a little-endian u16, four zero
//! bytes) and then entries: kind (1 byte), key (32), payload length (u32), CRC-32 of the kind, key,
//! length and payload (u32), payload. Integers are little-endian.

use alloc::string::String;
use alloc::vec::Vec;
use std::collections::HashMap;
use std::collections::hash_map::Entry;

use serde::{Deserialize, Serialize};
use wcontainers_gpu::{Reflection, ShaderModule, ShaderStage, VariantKey};

use crate::{CONVERTER_VERSION, ConvertError, Converted, Source, convert};

pub const MAGIC: [u8; 10] = *b"WCGPUCACHE";
/// The format version: a file of another version is not read.
pub const VERSION: u16 = 1;
const HEADER: usize = 16;
const ENTRY_HEAD: usize = 1 + 32 + 4 + 4;

/// What an entry holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Kind {
    /// A converted shader: [`shader_key`] to [`encode`]'s record.
    Shader = 1,
    /// A state combination a program used, for building ahead at the next start.
    Combination = 2,
}

/// Why a file was not read as a cache.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoadError {
    /// It does not start with the header.
    NotACache,
    /// It is a cache of another format version.
    Version(u16),
}

/// A cache file in memory: its bytes, which are what is written out, and an index from kind and key
/// to the payload.
#[derive(Clone, Debug)]
pub struct Cache {
    bytes: Vec<u8>,
    index: HashMap<(u8, [u8; 32]), (usize, usize)>,
    disagreements: u64,
}

impl Default for Cache {
    fn default() -> Self {
        let mut bytes = Vec::with_capacity(HEADER);
        bytes.extend_from_slice(&MAGIC);
        bytes.extend_from_slice(&VERSION.to_le_bytes());
        bytes.extend_from_slice(&[0; 4]);
        Cache {
            bytes,
            index: HashMap::new(),
            disagreements: 0,
        }
    }
}

impl Cache {
    /// Reads a cache file. An empty file is an empty cache. Reading stops at the first entry that is
    /// cut short or fails its check, and the bytes from there are dropped, so appending continues a
    /// well-formed file. Returns the cache and how many bytes were dropped.
    pub fn load(mut bytes: Vec<u8>) -> Result<(Cache, usize), LoadError> {
        if bytes.is_empty() {
            return Ok((Cache::default(), 0));
        }
        if bytes.len() < HEADER || bytes[..MAGIC.len()] != MAGIC {
            return Err(LoadError::NotACache);
        }
        let version = u16::from_le_bytes([bytes[10], bytes[11]]);
        if version != VERSION {
            return Err(LoadError::Version(version));
        }
        let mut cache = Cache::default();
        let mut at = HEADER;
        while let Some((kind, key, payload)) = entry(&bytes, at) {
            let end = payload.end;
            cache.index_entry(&bytes, kind, key, payload);
            at = end;
        }
        let dropped = bytes.len() - at;
        bytes.truncate(at);
        cache.bytes = bytes;
        Ok((cache, dropped))
    }

    /// The file's bytes: write them out to save or export the cache.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// How many distinct keys the cache holds.
    pub fn len(&self) -> usize {
        self.index.len()
    }

    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }

    /// Entries met whose key was already present with another payload (the first is kept). Keys
    /// are hashes of content, so any is damage or a converter that is not deterministic.
    pub fn disagreements(&self) -> u64 {
        self.disagreements
    }

    pub fn get(&self, kind: Kind, key: &[u8; 32]) -> Option<&[u8]> {
        self.index
            .get(&(kind as u8, *key))
            .map(|&(start, end)| &self.bytes[start..end])
    }

    /// Every entry as kind, key and payload, in the file's order, the first of each key only.
    pub fn entries(&self) -> impl Iterator<Item = (u8, [u8; 32], &[u8])> {
        let mut at = HEADER;
        core::iter::from_fn(move || {
            let (kind, key, payload) = entry(&self.bytes, at)?;
            at = payload.end;
            Some((kind, key, payload))
        })
        .filter(|(kind, key, payload)| {
            self.index.get(&(*kind, *key)) == Some(&(payload.start, payload.end))
        })
        .map(|(kind, key, payload)| (kind, key, &self.bytes[payload]))
    }

    /// Adds an entry. Returns the bytes appended to the file (empty when the key is already present),
    /// so a host keeping the file on disk appends them; a payload differing from the one present is
    /// counted in `disagreements` and dropped.
    pub fn insert(&mut self, kind: Kind, key: [u8; 32], payload: &[u8]) -> &[u8] {
        self.insert_raw(kind as u8, key, payload)
    }

    /// Adds every entry of `other` not present here: the union of the two files' entries.
    pub fn merge(&mut self, other: &Cache) {
        for (kind, key, payload) in other.entries() {
            self.insert_raw(kind, key, payload);
        }
    }

    fn insert_raw(&mut self, kind: u8, key: [u8; 32], payload: &[u8]) -> &[u8] {
        if let Some(&(start, end)) = self.index.get(&(kind, key)) {
            if self.bytes[start..end] != *payload {
                self.disagreements += 1;
            }
            return &[];
        }
        let length = u32::try_from(payload.len()).expect("a cache payload under 4 GiB");
        let at = self.bytes.len();
        self.bytes.push(kind);
        self.bytes.extend_from_slice(&key);
        self.bytes.extend_from_slice(&length.to_le_bytes());
        let mut crc = crc32fast::Hasher::new();
        crc.update(&self.bytes[at..]);
        crc.update(payload);
        self.bytes.extend_from_slice(&crc.finalize().to_le_bytes());
        self.bytes.extend_from_slice(payload);
        self.index
            .insert((kind, key), (at + ENTRY_HEAD, self.bytes.len()));
        &self.bytes[at..]
    }

    /// Indexes an entry of `bytes`, counting a key present with another payload.
    fn index_entry(
        &mut self,
        bytes: &[u8],
        kind: u8,
        key: [u8; 32],
        payload: core::ops::Range<usize>,
    ) {
        match self.index.entry((kind, key)) {
            Entry::Vacant(v) => {
                v.insert((payload.start, payload.end));
            }
            Entry::Occupied(o) => {
                let (start, end) = *o.get();
                if bytes[start..end] != bytes[payload] {
                    self.disagreements += 1;
                }
            }
        }
    }
}

/// The whole, checked entry at `at`: kind, key and the payload's range.
fn entry(bytes: &[u8], at: usize) -> Option<(u8, [u8; 32], core::ops::Range<usize>)> {
    let head = bytes.get(at..at.checked_add(ENTRY_HEAD)?)?;
    let length = u32::from_le_bytes(head[33..37].try_into().ok()?) as usize;
    let stored = u32::from_le_bytes(head[37..41].try_into().ok()?);
    let start = at + ENTRY_HEAD;
    let payload = bytes.get(start..start.checked_add(length)?)?;
    let mut crc = crc32fast::Hasher::new();
    crc.update(&head[..37]);
    crc.update(payload);
    (crc.finalize() == stored).then(|| {
        let key = head[1..33].try_into().expect("32 bytes");
        (head[0], key, start..start + length)
    })
}

/// The content key of a converted shader: the source bytes, the converter version and the variant.
pub fn shader_key(source: Source<'_>, variant: &VariantKey) -> [u8; 32] {
    let (tag, bytes): (u8, &[u8]) = match source {
        Source::Dxbc(b) => (0, b),
        Source::Dxil(b) => (1, b),
        Source::SpirV(w) => (2, words_as_bytes(w)),
    };
    let variant = postcard::to_allocvec(variant).expect("a variant key serialises");
    let mut hasher = blake3::Hasher::new();
    hasher.update(&[tag]);
    hasher.update(&(bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    hasher.update(&(CONVERTER_VERSION.len() as u64).to_le_bytes());
    hasher.update(CONVERTER_VERSION.as_bytes());
    hasher.update(&variant);
    *hasher.finalize().as_bytes()
}

fn words_as_bytes(words: &[u32]) -> &[u8] {
    // SAFETY: u32 has no padding, every byte pattern is a valid u8, and the length is the same memory.
    unsafe { core::slice::from_raw_parts(words.as_ptr().cast(), core::mem::size_of_val(words)) }
}

/// The record of a converted shader: each module with its entry point and stage, and the reflection.
#[derive(Serialize, Deserialize)]
struct ShaderRecord {
    modules: Vec<(naga::Module, String, ShaderStage)>,
    reflection: Reflection,
}

/// A converted shader as the cache keeps it.
pub fn encode(converted: &Converted) -> Vec<u8> {
    let record = ShaderRecord {
        modules: converted
            .modules
            .iter()
            .map(|m| (m.module.clone(), m.entry_point.clone(), m.stage))
            .collect(),
        reflection: converted.reflection.clone(),
    };
    postcard::to_allocvec(&record).expect("a converted shader serialises")
}

/// A converted shader from its record, its modules validated again and given `key`.
pub fn decode(key: [u8; 32], payload: &[u8]) -> Result<Converted, ConvertError> {
    let record: ShaderRecord = postcard::from_bytes(payload)
        .map_err(|e| ConvertError::Malformed(alloc::format!("cache record: {e}")))?;
    let modules = record
        .modules
        .into_iter()
        .enumerate()
        .map(|(i, (module, entry_point, stage))| {
            Ok(ShaderModule {
                info: crate::validate(&module)?,
                module,
                entry_point,
                stage,
                key: module_key(key, i),
            })
        })
        .collect::<Result<_, ConvertError>>()?;
    Ok(Converted {
        modules,
        reflection: record.reflection,
    })
}

/// The content key of module `index` of a conversion whose key is `key`: the key itself for the first,
/// so each module a back end builds from has its own.
pub(crate) fn module_key(key: [u8; 32], index: usize) -> [u8; 32] {
    if index == 0 {
        return key;
    }
    *blake3::Hasher::new()
        .update(&key)
        .update(&(index as u32).to_le_bytes())
        .finalize()
        .as_bytes()
}

/// `convert` through the cache: a recorded conversion is decoded, anything else converted and
/// recorded. Returns the bytes appended to the file, if any, beside the result.
pub fn convert_cached(
    cache: &mut Cache,
    source: Source<'_>,
    variant: &VariantKey,
    root_signature: Option<&[u8]>,
) -> Result<(Converted, Vec<u8>), ConvertError> {
    let key = shader_key(source, variant);
    if let Some(payload) = cache.get(Kind::Shader, &key) {
        return decode(key, payload).map(|c| (c, Vec::new()));
    }
    let converted = convert(source, variant, root_signature)?;
    let appended = cache
        .insert(Kind::Shader, key, &encode(&converted))
        .to_vec();
    Ok((converted, appended))
}

/// Records a state combination a program was drawn with, in the front's own terms (its blocks, the
/// program's shader keys and variant keys, target layout, topology class and binding layout keys);
/// its key is the hash of its record. Returns the bytes appended to the file, if any.
pub fn record_combination<T: Serialize>(cache: &mut Cache, combination: &T) -> Vec<u8> {
    let record = postcard::to_allocvec(combination).expect("a combination serialises");
    let key = *blake3::hash(&record).as_bytes();
    cache.insert(Kind::Combination, key, &record).to_vec()
}

/// Every recorded state combination, in the order recorded, for building ahead at start-up. A record
/// that does not decode as `T` (another front's, or another version's) is skipped.
pub fn combinations<'a, T: Deserialize<'a>>(cache: &'a Cache) -> impl Iterator<Item = T> + 'a {
    cache
        .entries()
        .filter(|&(kind, _, _)| kind == Kind::Combination as u8)
        .filter_map(|(_, _, record)| postcard::from_bytes(record).ok())
}

impl core::fmt::Display for LoadError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            LoadError::NotACache => f.write_str("not a cache file"),
            LoadError::Version(v) => write!(f, "cache format version {v}, not {VERSION}"),
        }
    }
}
