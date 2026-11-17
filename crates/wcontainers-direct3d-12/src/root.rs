//! Root signatures in their serialized form: a DXBC container holding one `RTS0` part, as
//! `D3D12SerializeVersionedRootSignature` writes, `CreateRootSignature` reads and shader compilers
//! embed in DXIL.

/// `D3D12_ROOT_PARAMETER_TYPE` with its payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RootParameterKind {
    Table(Vec<DescriptorRange>),
    Constants {
        register: u32,
        space: u32,
        count: u32,
    },
    /// A root descriptor: `kind` 2 (constant buffer view), 3 (shader resource view) or 4 (unordered
    /// access view).
    Descriptor {
        kind: u32,
        register: u32,
        space: u32,
        flags: u32,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RootParameter {
    pub kind: RootParameterKind,
    /// `D3D12_SHADER_VISIBILITY`.
    pub visibility: u32,
}

/// `D3D12_DESCRIPTOR_RANGE1`; ranges of version 1.0 have flags 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DescriptorRange {
    /// `D3D12_DESCRIPTOR_RANGE_TYPE`: 0 SRV, 1 UAV, 2 CBV, 3 sampler.
    pub kind: u32,
    /// `u32::MAX` for unbounded.
    pub count: u32,
    pub register: u32,
    pub space: u32,
    pub flags: u32,
    /// `u32::MAX` to follow the previous range.
    pub offset: u32,
}

/// `D3D12_STATIC_SAMPLER_DESC`, its 13 fields as written (floats as bits).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StaticSampler(pub [u32; 13]);

impl StaticSampler {
    pub fn register(&self) -> u32 {
        self.0[10]
    }
    pub fn space(&self) -> u32 {
        self.0[11]
    }
}

/// A root signature of version 1.0 (`version` 1) or 1.1 (`version` 2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RootSignatureDesc {
    pub version: u32,
    pub parameters: Vec<RootParameter>,
    pub samplers: Vec<StaticSampler>,
    /// `D3D12_ROOT_SIGNATURE_FLAGS`.
    pub flags: u32,
}

/// `D3D12_DESCRIPTOR_RANGE_FLAG_DESCRIPTORS_VOLATILE`.
const DESCRIPTORS_VOLATILE: u32 = 1;
/// `D3D12_DESCRIPTOR_RANGE_FLAG_DATA_VOLATILE` and `D3D12_ROOT_DESCRIPTOR_FLAG_DATA_VOLATILE`.
const DATA_VOLATILE: u32 = 2;

impl RootSignatureDesc {
    /// This signature at `version` (1 for 1.0, 2 for 1.1), as the versioned deserializer converts:
    /// to 1.0 the flags go; to 1.1 each range and root descriptor of a 1.0 signature gets the flags
    /// that say 1.0's behaviour (descriptors and data volatile; sampler ranges descriptors only).
    pub fn at_version(&self, version: u32) -> RootSignatureDesc {
        let mut out = self.clone();
        if version == self.version {
            return out;
        }
        out.version = version;
        for p in &mut out.parameters {
            match &mut p.kind {
                RootParameterKind::Table(ranges) => {
                    for r in ranges {
                        r.flags = match (version, r.kind) {
                            (1, _) => 0,
                            (_, 3) => DESCRIPTORS_VOLATILE,
                            _ => DESCRIPTORS_VOLATILE | DATA_VOLATILE,
                        };
                    }
                }
                RootParameterKind::Descriptor { flags, .. } => {
                    *flags = if version == 1 { 0 } else { DATA_VOLATILE };
                }
                RootParameterKind::Constants { .. } => {}
            }
        }
        out
    }

    /// Direct3D 12's rules for a signature `CreateRootSignature` accepts: ranges are not empty and
    /// end within the register numbers, samplers have tables of their own, a table's offsets fit and
    /// do not follow an unbounded range, no register is bound twice for one stage, and the root
    /// arguments fit in 64 dwords (a table costs one, a root descriptor two, constants one each).
    pub fn validate(&self) -> Result<(), &'static str> {
        /// A register range: class (0 SRV, 1 UAV, 2 CBV, 3 sampler), space, first, last, visibility.
        type Span = (u32, u32, u32, u32, u32);
        let cost = self.parameters.iter().fold(0u64, |cost, p| {
            cost + match &p.kind {
                RootParameterKind::Table(_) => 1,
                RootParameterKind::Constants { count, .. } => u64::from(*count),
                RootParameterKind::Descriptor { .. } => 2,
            }
        });
        if cost > 64 {
            return Err("root arguments of more than 64 dwords");
        }
        let mut spans: Vec<Span> = Vec::new();
        for p in &self.parameters {
            let v = p.visibility;
            match &p.kind {
                RootParameterKind::Table(ranges) => {
                    let samplers = ranges.iter().filter(|r| r.kind == 3).count();
                    if samplers != 0 && samplers != ranges.len() {
                        return Err("samplers in a table with other descriptors");
                    }
                    let mut end = 0u64;
                    for r in ranges {
                        if r.count == 0 {
                            return Err("an empty descriptor range");
                        }
                        let unbounded = r.count == u32::MAX;
                        let last = if unbounded {
                            u32::MAX
                        } else {
                            r.register
                                .checked_add(r.count - 1)
                                .ok_or("a descriptor range past the last register")?
                        };
                        let start = if r.offset == u32::MAX {
                            if end == u64::MAX {
                                return Err("a range appended after an unbounded one");
                            }
                            end
                        } else {
                            u64::from(r.offset)
                        };
                        end = if unbounded {
                            u64::MAX
                        } else {
                            start + u64::from(r.count)
                        };
                        if end != u64::MAX && end > u64::from(u32::MAX) {
                            return Err("a descriptor range past the table's offsets");
                        }
                        spans.push((r.kind, r.space, r.register, last, v));
                    }
                }
                RootParameterKind::Constants {
                    register, space, ..
                } => spans.push((2, *space, *register, *register, v)),
                RootParameterKind::Descriptor {
                    kind,
                    register,
                    space,
                    ..
                } => {
                    let class = match kind {
                        2 => 2,
                        3 => 0,
                        _ => 1,
                    };
                    spans.push((class, *space, *register, *register, v));
                }
            }
        }
        for s in &self.samplers {
            spans.push((3, s.space(), s.register(), s.register(), s.0[12]));
        }
        for (i, a) in spans.iter().enumerate() {
            for b in &spans[i + 1..] {
                let seen_together = a.4 == 0 || b.4 == 0 || a.4 == b.4;
                if a.0 == b.0 && a.1 == b.1 && a.2 <= b.3 && b.2 <= a.3 && seen_together {
                    return Err("a register bound twice for one stage");
                }
            }
        }
        Ok(())
    }
}

const RTS0: [u8; 4] = *b"RTS0";

/// Reads a serialized root signature. `Err` says why it is malformed (`E_INVALIDARG`).
pub fn deserialize(blob: &[u8]) -> Result<RootSignatureDesc, &'static str> {
    let part = rts0_part(blob)?;
    let word = |at: usize| -> Result<u32, &'static str> {
        part.get(at..at + 4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
            .ok_or("a root signature field is past its part")
    };
    let version = word(0)?;
    if !matches!(version, 1 | 2) {
        return Err("unknown root signature version");
    }
    let (count, at, samplers, samplers_at, flags) =
        (word(4)?, word(8)?, word(12)?, word(16)?, word(20)?);
    let mut parameters = Vec::new();
    for i in 0..count as usize {
        let p = at as usize + i * 12;
        let (kind, visibility, payload) = (word(p)?, word(p + 4)?, word(p + 8)? as usize);
        let kind = match kind {
            0 => {
                let (n, ranges_at) = (word(payload)?, word(payload + 4)? as usize);
                let size = if version == 1 { 20 } else { 24 };
                let mut ranges = Vec::new();
                for r in 0..n as usize {
                    let q = ranges_at + r * size;
                    let flags = if version == 1 { 0 } else { word(q + 16)? };
                    ranges.push(DescriptorRange {
                        kind: word(q)?,
                        count: word(q + 4)?,
                        register: word(q + 8)?,
                        space: word(q + 12)?,
                        flags,
                        offset: word(q + size - 4)?,
                    });
                    if ranges[r].kind > 3 {
                        return Err("unknown descriptor range type");
                    }
                }
                RootParameterKind::Table(ranges)
            }
            1 => RootParameterKind::Constants {
                register: word(payload)?,
                space: word(payload + 4)?,
                count: word(payload + 8)?,
            },
            2..=4 => RootParameterKind::Descriptor {
                kind,
                register: word(payload)?,
                space: word(payload + 4)?,
                flags: if version == 1 { 0 } else { word(payload + 8)? },
            },
            _ => return Err("unknown root parameter type"),
        };
        parameters.push(RootParameter { kind, visibility });
    }
    let mut statics = Vec::new();
    for i in 0..samplers as usize {
        let s = samplers_at as usize + i * 52;
        let mut fields = [0; 13];
        for (j, f) in fields.iter_mut().enumerate() {
            *f = word(s + j * 4)?;
        }
        statics.push(StaticSampler(fields));
    }
    Ok(RootSignatureDesc {
        version,
        parameters,
        samplers: statics,
        flags,
    })
}

/// The `RTS0` part of a DXBC container.
fn rts0_part(blob: &[u8]) -> Result<&[u8], &'static str> {
    let word = |at: usize| -> Result<usize, &'static str> {
        blob.get(at..at + 4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()) as usize)
            .ok_or("the container is cut short")
    };
    if blob.get(..4) != Some(b"DXBC") {
        return Err("not a DXBC container");
    }
    for i in 0..word(28)? {
        let at = word(32 + i * 4)?;
        let size = word(at + 4)?;
        if blob.get(at..at + 4) == Some(&RTS0) {
            return blob
                .get(at + 8..at + 8 + size)
                .ok_or("the root signature part is cut short");
        }
    }
    Err("the container has no root signature part")
}

/// The checksum of a DXBC container: MD5's rounds over the bytes after the checksum, with DXBC's
/// own last block (the bit count first, then the rest of the bytes, MD5's padding, and the bit
/// count divided by 4 with its low bit set), no MD5 finalisation.
pub fn dxbc_checksum(container: &[u8]) -> [u8; 16] {
    let bytes = &container[20.min(container.len())..];
    let bits = (bytes.len() as u32).wrapping_mul(8);
    let (a, b) = (bits.to_le_bytes(), ((bits >> 2) | 1).to_le_bytes());
    let whole = bytes.len() - bytes.len() % 64;
    let mut state = [0x6745_2301, 0xefcd_ab89, 0x98ba_dcfe, 0x1032_5476];
    md5::block_api::compress(&mut state, bytes[..whole].as_chunks::<64>().0);
    let rest = &bytes[whole..];
    let mut last = [[0u8; 64]; 2];
    let tail = if rest.len() >= 56 {
        last[0][..rest.len()].copy_from_slice(rest);
        last[0][rest.len()] = 0x80;
        last[1][..4].copy_from_slice(&a);
        &mut last[..]
    } else {
        last[0][..4].copy_from_slice(&a);
        last[0][4..4 + rest.len()].copy_from_slice(rest);
        last[0][4 + rest.len()] = 0x80;
        &mut last[..1]
    };
    tail.last_mut().expect("a block")[60..].copy_from_slice(&b);
    md5::block_api::compress(&mut state, tail);
    let mut out = [0; 16];
    for (o, s) in out.as_chunks_mut::<4>().0.iter_mut().zip(state) {
        *o = s.to_le_bytes();
    }
    out
}

/// Writes a root signature as a DXBC container with one `RTS0` part, its checksum as Direct3D
/// writes it.
pub fn serialize(desc: &RootSignatureDesc) -> Vec<u8> {
    let mut part = vec![0u32; 6];
    part[0] = desc.version;
    part[1] = desc.parameters.len() as u32;
    part[2] = 24;
    part[3] = desc.samplers.len() as u32;
    part[5] = desc.flags;
    let headers = part.len();
    part.resize(headers + desc.parameters.len() * 3, 0);
    for (i, p) in desc.parameters.iter().enumerate() {
        let h = headers + i * 3;
        let payload = (part.len() * 4) as u32;
        let kind = match &p.kind {
            RootParameterKind::Table(ranges) => {
                let ranges_at = payload + 8;
                part.extend([ranges.len() as u32, ranges_at]);
                for r in ranges {
                    part.extend([r.kind, r.count, r.register, r.space]);
                    if desc.version != 1 {
                        part.push(r.flags);
                    }
                    part.push(r.offset);
                }
                0
            }
            RootParameterKind::Constants {
                register,
                space,
                count,
            } => {
                part.extend([*register, *space, *count]);
                1
            }
            RootParameterKind::Descriptor {
                kind,
                register,
                space,
                flags,
            } => {
                part.extend([*register, *space]);
                if desc.version != 1 {
                    part.push(*flags);
                }
                *kind
            }
        };
        part[h..h + 3].copy_from_slice(&[kind, p.visibility, payload]);
    }
    part[4] = (part.len() * 4) as u32;
    for s in &desc.samplers {
        part.extend(s.0);
    }
    let data: Vec<u8> = part.iter().flat_map(|w| w.to_le_bytes()).collect();
    let mut blob = Vec::with_capacity(44 + data.len());
    blob.extend(b"DXBC");
    blob.extend([0u8; 16]);
    let total = 36 + 8 + data.len() as u32;
    for w in [1u32, total, 1, 36] {
        blob.extend(w.to_le_bytes());
    }
    blob.extend(RTS0);
    blob.extend((data.len() as u32).to_le_bytes());
    blob.extend(data);
    let checksum = dxbc_checksum(&blob);
    blob[4..20].copy_from_slice(&checksum);
    blob
}
