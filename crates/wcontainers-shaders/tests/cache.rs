//! The cache file (plan section 8): it round-trips, merges by union in either order, and loads every
//! whole entry before a cut or a damaged entry. Cuts are seeded: `WCONTAINERS_CACHE_SEED` (default 1)
//! and `WCONTAINERS_CACHE_CUTS` (how many places, default 10).

use std::collections::BTreeMap;
use std::path::Path;
use wcontainers_gpu::VariantKey;
use wcontainers_shaders::Source;
use wcontainers_shaders::cache::{self, Cache, Kind, LoadError};

fn number(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .map(|v| {
            v.parse()
                .unwrap_or_else(|_| panic!("{name} is not a number"))
        })
        .unwrap_or(default)
}

fn own(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/own")
            .join(name),
    )
    .unwrap()
}

/// Every entry as a map, the form two caches holding the same entries share.
fn contents(c: &Cache) -> BTreeMap<(u8, [u8; 32]), Vec<u8>> {
    c.entries()
        .map(|(k, key, p)| ((k, key), p.to_vec()))
        .collect()
}

fn key(n: u8) -> [u8; 32] {
    *blake3::hash(&[n]).as_bytes()
}

/// A cache of `n` entries with payloads of varied length, from `first` on.
fn made(first: u8, n: u8) -> Cache {
    let mut c = Cache::default();
    for i in first..first + n {
        let payload: Vec<u8> = (0..u32::from(i) * 7 % 50).map(|b| b as u8 ^ i).collect();
        let kind = if i % 3 == 0 {
            Kind::Combination
        } else {
            Kind::Shader
        };
        assert!(!c.insert(kind, key(i), &payload).is_empty());
    }
    c
}

#[test]
fn converted_shaders_round_trip_through_the_file() {
    let mut c = Cache::default();
    let mut appended = c.bytes().to_vec();
    let mut keys = Vec::new();
    for name in [
        "vs_transform.dxbc",
        "ps_textured.dxbc",
        "ps_negate.dxil",
        "vs_carry.dxil",
    ] {
        let bytes = own(name);
        let source = if name.ends_with("dxil") {
            Source::Dxil(&bytes)
        } else {
            Source::Dxbc(&bytes)
        };
        let variant = VariantKey::default();
        let (converted, added) = cache::convert_cached(&mut c, source, &variant, None).unwrap();
        assert!(!added.is_empty(), "{name}: a first conversion is recorded");
        appended.extend_from_slice(&added);
        let key = cache::shader_key(source, &variant);
        assert_eq!(converted.modules[0].key, key, "{name}");
        // Again: from the cache, nothing appended, the same record.
        let (again, added) = cache::convert_cached(&mut c, source, &variant, None).unwrap();
        assert!(added.is_empty(), "{name}");
        assert_eq!(cache::encode(&again), cache::encode(&converted), "{name}");
        assert_eq!(again.reflection, converted.reflection, "{name}");
        keys.push((name, key, cache::encode(&converted)));
    }
    // What a host appended to its file is the file.
    assert_eq!(appended, c.bytes());
    let (loaded, dropped) = Cache::load(c.bytes().to_vec()).unwrap();
    assert_eq!(dropped, 0);
    assert_eq!(loaded.bytes(), c.bytes());
    assert_eq!(loaded.len(), 4);
    for (name, key, record) in keys {
        let payload = loaded.get(Kind::Shader, &key).expect(name);
        assert_eq!(payload, record.as_slice(), "{name}");
        let decoded = cache::decode(key, payload).unwrap();
        assert_eq!(cache::encode(&decoded), record, "{name}");
    }
}

#[test]
fn caches_merge_by_union_in_either_order() {
    let (a, b) = (made(0, 12), made(8, 12));
    let mut ab = a.clone();
    ab.merge(&b);
    let mut ba = b.clone();
    ba.merge(&a);
    assert_eq!(contents(&ab), contents(&ba));
    assert_eq!(ab.len(), 20);
    let mut union = contents(&a);
    union.extend(contents(&b));
    assert_eq!(contents(&ab), union);
    assert_eq!((ab.disagreements(), ba.disagreements()), (0, 0));
    // A merged file reads back as itself.
    let (loaded, dropped) = Cache::load(ab.bytes().to_vec()).unwrap();
    assert_eq!((contents(&loaded), dropped), (contents(&ab), 0));
    // Merging is idempotent.
    let mut again = ab.clone();
    again.merge(&ba);
    assert_eq!(again.bytes(), ab.bytes());
}

#[test]
fn a_disagreeing_payload_is_counted_and_the_first_kept() {
    let mut a = made(0, 4);
    let mut b = Cache::default();
    b.insert(Kind::Shader, key(1), b"other");
    a.merge(&b);
    assert_eq!(a.disagreements(), 1);
    assert_ne!(a.get(Kind::Shader, &key(1)), Some(&b"other"[..]));
    // Within one file, as appends of two writers would leave it.
    let mut bytes = made(0, 4).bytes().to_vec();
    bytes.extend_from_slice(&b.bytes()[16..]);
    let (loaded, dropped) = Cache::load(bytes).unwrap();
    assert_eq!((loaded.disagreements(), dropped, loaded.len()), (1, 0, 4));
}

#[test]
fn a_cut_file_keeps_every_whole_entry_before_the_cut() {
    let seed = number("WCONTAINERS_CACHE_SEED", 1);
    let cuts = number("WCONTAINERS_CACHE_CUTS", 10);
    let full = made(0, 30);
    let bytes = full.bytes().to_vec();
    // Where each entry ends, in file order.
    let mut ends = Vec::new();
    let mut so_far = Cache::default();
    for (kind, key, payload) in full.entries() {
        let kind = if kind == Kind::Shader as u8 {
            Kind::Shader
        } else {
            Kind::Combination
        };
        so_far.insert(kind, key, payload);
        ends.push(so_far.bytes().len());
    }
    let mut rng = fastrand::Rng::with_seed(seed);
    for cut in 0..cuts {
        let at = rng.usize(16..bytes.len());
        let (mut c, dropped) = Cache::load(bytes[..at].to_vec()).unwrap();
        let whole = ends.iter().filter(|&&e| e <= at).count();
        let kept = ends[..whole].last().copied().unwrap_or(16);
        eprintln!(
            "seed {seed} cut {cut}: at {at} of {}, {whole} entries kept",
            bytes.len()
        );
        assert_eq!(c.len(), whole, "seed {seed} cut {cut} at {at}");
        assert_eq!(dropped, at - kept, "seed {seed} cut {cut} at {at}");
        assert_eq!(c.bytes(), &bytes[..kept], "seed {seed} cut {cut} at {at}");
        // Appending after a cut gives a file that loads whole.
        c.insert(Kind::Shader, key(200), b"after the cut");
        let (again, dropped) = Cache::load(c.bytes().to_vec()).unwrap();
        assert_eq!((again.len(), dropped), (whole + 1, 0));
    }
    // A damaged byte inside an entry ends the file there.
    let mut damaged = bytes.clone();
    damaged[ends[9] + 50] ^= 1;
    let (c, dropped) = Cache::load(damaged).unwrap();
    assert_eq!((c.len(), dropped), (10, bytes.len() - ends[9]));
}

#[test]
fn other_files_are_not_read_as_caches() {
    assert_eq!(
        Cache::load(b"not a cache file".to_vec()).unwrap_err(),
        LoadError::NotACache
    );
    let mut bytes = Cache::default().bytes().to_vec();
    bytes[10] = 9;
    assert_eq!(Cache::load(bytes).unwrap_err(), LoadError::Version(9));
    let (c, dropped) = Cache::load(Vec::new()).unwrap();
    assert_eq!((c.len(), dropped, c.bytes().len()), (0, 0, 16));
}
