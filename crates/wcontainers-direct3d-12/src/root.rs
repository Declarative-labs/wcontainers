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

/// Writes a root signature as a DXBC container with one `RTS0` part. The container's checksum is
/// left zero: only this runtime reads what it writes, and it does not check it.
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
    blob
}
