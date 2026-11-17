//! Root signature deserializers: a serialized root signature read back into the API's structures.

use wcontainers_direct3d_12::root::{RootParameterKind, RootSignatureDesc, StaticSampler};
use windows::Win32::Foundation::E_INVALIDARG;
use windows::Win32::Graphics::Direct3D12::*;
use windows::core::{Result, implement};

/// The API's structures of one root signature at one version; the description points into the
/// vectors, which never change after construction.
struct Tables {
    desc: D3D12_VERSIONED_ROOT_SIGNATURE_DESC,
    _ranges0: Vec<Vec<D3D12_DESCRIPTOR_RANGE>>,
    _ranges1: Vec<Vec<D3D12_DESCRIPTOR_RANGE1>>,
    _params0: Vec<D3D12_ROOT_PARAMETER>,
    _params1: Vec<D3D12_ROOT_PARAMETER1>,
    _samplers: Vec<D3D12_STATIC_SAMPLER_DESC>,
}

fn sampler(s: &StaticSampler) -> D3D12_STATIC_SAMPLER_DESC {
    let w = s.0;
    D3D12_STATIC_SAMPLER_DESC {
        Filter: D3D12_FILTER(w[0] as i32),
        AddressU: D3D12_TEXTURE_ADDRESS_MODE(w[1] as i32),
        AddressV: D3D12_TEXTURE_ADDRESS_MODE(w[2] as i32),
        AddressW: D3D12_TEXTURE_ADDRESS_MODE(w[3] as i32),
        MipLODBias: f32::from_bits(w[4]),
        MaxAnisotropy: w[5],
        ComparisonFunc: D3D12_COMPARISON_FUNC(w[6] as i32),
        BorderColor: D3D12_STATIC_BORDER_COLOR(w[7] as i32),
        MinLOD: f32::from_bits(w[8]),
        MaxLOD: f32::from_bits(w[9]),
        ShaderRegister: w[10],
        RegisterSpace: w[11],
        ShaderVisibility: D3D12_SHADER_VISIBILITY(w[12] as i32),
    }
}

impl Tables {
    /// `desc` in the API's structures of its own version (1 for 1.0, 2 for 1.1).
    fn new(desc: &RootSignatureDesc) -> Box<Tables> {
        let v1 = desc.version != 1;
        let mut t = Box::new(Tables {
            desc: D3D12_VERSIONED_ROOT_SIGNATURE_DESC::default(),
            _ranges0: Vec::new(),
            _ranges1: Vec::new(),
            _params0: Vec::new(),
            _params1: Vec::new(),
            _samplers: desc.samplers.iter().map(sampler).collect(),
        });
        for p in &desc.parameters {
            if let RootParameterKind::Table(ranges) = &p.kind {
                if v1 {
                    t._ranges1.push(
                        ranges
                            .iter()
                            .map(|r| D3D12_DESCRIPTOR_RANGE1 {
                                RangeType: D3D12_DESCRIPTOR_RANGE_TYPE(r.kind as i32),
                                NumDescriptors: r.count,
                                BaseShaderRegister: r.register,
                                RegisterSpace: r.space,
                                Flags: D3D12_DESCRIPTOR_RANGE_FLAGS(r.flags as i32),
                                OffsetInDescriptorsFromTableStart: r.offset,
                            })
                            .collect(),
                    );
                } else {
                    t._ranges0.push(
                        ranges
                            .iter()
                            .map(|r| D3D12_DESCRIPTOR_RANGE {
                                RangeType: D3D12_DESCRIPTOR_RANGE_TYPE(r.kind as i32),
                                NumDescriptors: r.count,
                                BaseShaderRegister: r.register,
                                RegisterSpace: r.space,
                                OffsetInDescriptorsFromTableStart: r.offset,
                            })
                            .collect(),
                    );
                }
            }
        }
        let mut table = 0;
        for p in &desc.parameters {
            let visibility = D3D12_SHADER_VISIBILITY(p.visibility as i32);
            let (kind, register, space, values, flags) = match &p.kind {
                RootParameterKind::Table(_) => (0, 0, 0, 0, 0),
                RootParameterKind::Constants {
                    register,
                    space,
                    count,
                } => (1, *register, *space, *count, 0),
                RootParameterKind::Descriptor {
                    kind,
                    register,
                    space,
                    flags,
                } => (*kind, *register, *space, 0, *flags),
            };
            let constants = D3D12_ROOT_CONSTANTS {
                ShaderRegister: register,
                RegisterSpace: space,
                Num32BitValues: values,
            };
            if v1 {
                let anonymous = match kind {
                    0 => {
                        let r = &t._ranges1[table];
                        table += 1;
                        D3D12_ROOT_PARAMETER1_0 {
                            DescriptorTable: D3D12_ROOT_DESCRIPTOR_TABLE1 {
                                NumDescriptorRanges: r.len() as u32,
                                pDescriptorRanges: r.as_ptr(),
                            },
                        }
                    }
                    1 => D3D12_ROOT_PARAMETER1_0 {
                        Constants: constants,
                    },
                    _ => D3D12_ROOT_PARAMETER1_0 {
                        Descriptor: D3D12_ROOT_DESCRIPTOR1 {
                            ShaderRegister: register,
                            RegisterSpace: space,
                            Flags: D3D12_ROOT_DESCRIPTOR_FLAGS(flags as i32),
                        },
                    },
                };
                t._params1.push(D3D12_ROOT_PARAMETER1 {
                    ParameterType: D3D12_ROOT_PARAMETER_TYPE(kind as i32),
                    Anonymous: anonymous,
                    ShaderVisibility: visibility,
                });
            } else {
                let anonymous = match kind {
                    0 => {
                        let r = &t._ranges0[table];
                        table += 1;
                        D3D12_ROOT_PARAMETER_0 {
                            DescriptorTable: D3D12_ROOT_DESCRIPTOR_TABLE {
                                NumDescriptorRanges: r.len() as u32,
                                pDescriptorRanges: r.as_ptr(),
                            },
                        }
                    }
                    1 => D3D12_ROOT_PARAMETER_0 {
                        Constants: constants,
                    },
                    _ => D3D12_ROOT_PARAMETER_0 {
                        Descriptor: D3D12_ROOT_DESCRIPTOR {
                            ShaderRegister: register,
                            RegisterSpace: space,
                        },
                    },
                };
                t._params0.push(D3D12_ROOT_PARAMETER {
                    ParameterType: D3D12_ROOT_PARAMETER_TYPE(kind as i32),
                    Anonymous: anonymous,
                    ShaderVisibility: visibility,
                });
            }
        }
        let samplers = (t._samplers.len() as u32, t._samplers.as_ptr());
        let flags = D3D12_ROOT_SIGNATURE_FLAGS(desc.flags as i32);
        t.desc = if v1 {
            D3D12_VERSIONED_ROOT_SIGNATURE_DESC {
                Version: D3D_ROOT_SIGNATURE_VERSION_1_1,
                Anonymous: D3D12_VERSIONED_ROOT_SIGNATURE_DESC_0 {
                    Desc_1_1: D3D12_ROOT_SIGNATURE_DESC1 {
                        NumParameters: t._params1.len() as u32,
                        pParameters: t._params1.as_ptr(),
                        NumStaticSamplers: samplers.0,
                        pStaticSamplers: samplers.1,
                        Flags: flags,
                    },
                },
            }
        } else {
            D3D12_VERSIONED_ROOT_SIGNATURE_DESC {
                Version: D3D_ROOT_SIGNATURE_VERSION_1_0,
                Anonymous: D3D12_VERSIONED_ROOT_SIGNATURE_DESC_0 {
                    Desc_1_0: D3D12_ROOT_SIGNATURE_DESC {
                        NumParameters: t._params0.len() as u32,
                        pParameters: t._params0.as_ptr(),
                        NumStaticSamplers: samplers.0,
                        pStaticSamplers: samplers.1,
                        Flags: flags,
                    },
                },
            }
        };
        t
    }
}

/// `ID3D12RootSignatureDeserializer`: the signature at version 1.0.
#[implement(ID3D12RootSignatureDeserializer)]
pub struct Deserializer(Box<Tables>);

impl Deserializer {
    pub fn new(desc: &RootSignatureDesc) -> Self {
        Deserializer(Tables::new(&desc.at_version(1)))
    }
}

impl ID3D12RootSignatureDeserializer_Impl for Deserializer_Impl {
    fn GetRootSignatureDesc(&self) -> *mut D3D12_ROOT_SIGNATURE_DESC {
        // SAFETY: a 1.0 description.
        unsafe { &self.0.desc.Anonymous.Desc_1_0 as *const _ as *mut _ }
    }
}

/// `ID3D12VersionedRootSignatureDeserializer`: the signature as serialized, and at 1.0 and 1.1.
#[implement(ID3D12VersionedRootSignatureDeserializer)]
pub struct VersionedDeserializer {
    unconverted: Box<Tables>,
    at: [Box<Tables>; 2],
}

impl VersionedDeserializer {
    pub fn new(desc: &RootSignatureDesc) -> Self {
        VersionedDeserializer {
            unconverted: Tables::new(desc),
            at: [1, 2].map(|v| Tables::new(&desc.at_version(v))),
        }
    }
}

impl ID3D12VersionedRootSignatureDeserializer_Impl for VersionedDeserializer_Impl {
    fn GetRootSignatureDescAtVersion(
        &self,
        version: D3D_ROOT_SIGNATURE_VERSION,
    ) -> Result<*mut D3D12_VERSIONED_ROOT_SIGNATURE_DESC> {
        let t = match version {
            D3D_ROOT_SIGNATURE_VERSION_1_0 => &self.at[0],
            D3D_ROOT_SIGNATURE_VERSION_1_1 => &self.at[1],
            _ => return Err(E_INVALIDARG.into()),
        };
        Ok(&t.desc as *const _ as *mut _)
    }

    fn GetUnconvertedRootSignatureDesc(&self) -> *mut D3D12_VERSIONED_ROOT_SIGNATURE_DESC {
        &self.unconverted.desc as *const _ as *mut _
    }
}
