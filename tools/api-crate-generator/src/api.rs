//! The hand-written input: which part of the metadata each API crate is, and the mapping rule's exception lines.
//! Everything else the generator writes follows MAPPING.md from the metadata alone.

/// One outside API, one crate.
pub struct Api {
    /// The crate's folder under crates/, the API's name.
    pub folder: &'static str,
    /// The top-level trait, named for the folder: the library, its exports, and one associated type per object kind.
    pub top_trait: Option<&'static str>,
    pub summary: &'static str,
    /// Metadata namespaces whose every interface, record and constant belongs to this crate.
    pub namespaces: &'static [&'static str],
    /// Metadata namespaces of which this crate holds only what the other crates of the set name.
    pub referenced_namespaces: &'static [&'static str],
    /// The library whose exports are the top-level trait's methods.
    pub library: Option<&'static str>,
    pub function_prefixes: &'static [&'static str],
    pub type_prefixes: &'static [&'static str],
    pub interface_prefixes: &'static [&'static str],
    /// Interfaces of these namespaces that belong to another library (another API), with the reason.
    pub left_out: &'static [&'static str],
    /// Interfaces only ever met as the base of other interfaces: no object is of exactly this kind.
    pub abstract_bases: &'static [&'static str],
    /// The other crates of the set this crate's interface names.
    pub depends_on: &'static [&'static str],
    /// The other APIs whose top-level traits are supertraits of this one's: its objects answer their interfaces.
    pub supertraits: &'static [&'static str],
    /// Other interfaces an object kind answers through QueryInterface, as (kind's root interface, interfaces).
    pub faces: &'static [(&'static str, &'static [&'static str])],
}

pub const COMPONENT_OBJECT_MODEL: Api = Api {
    folder: "component-object-model",
    top_trait: None,
    summary: "The identifiers and status codes of COM, and the Win32 plain data the COM graphics APIs pass.",
    namespaces: &[],
    referenced_namespaces: &[
        "Windows.Win32.Foundation",
        "Windows.Win32.Graphics.Gdi",
        "Windows.Win32.Security",
        "Windows.Win32.System.Com",
    ],
    library: None,
    function_prefixes: &[],
    type_prefixes: &[],
    interface_prefixes: &["I"],
    left_out: &[],
    abstract_bases: &[],
    depends_on: &[],
    supertraits: &[],
    faces: &[],
};

pub const DIRECT3D_COMMON: Api = Api {
    folder: "direct3d-common",
    top_trait: Some("Direct3DCommon"),
    summary: "The part of Direct3D that versions 10, 11 and 12 share, from d3dcommon.h.",
    namespaces: &["Windows.Win32.Graphics.Direct3D"],
    referenced_namespaces: &[],
    library: None,
    function_prefixes: &[],
    type_prefixes: &["D3D_", "D3D10_", "D3D11_", "D3D12_"],
    interface_prefixes: &["ID3D"],
    // ID3DInclude is the shader compiler's callback (d3dcompiler_47.dll), and not a COM interface.
    left_out: &["ID3DInclude"],
    abstract_bases: &[],
    depends_on: &["component-object-model"],
    supertraits: &[],
    faces: &[],
};

pub const DIRECTX_GRAPHICS_INFRASTRUCTURE: Api = Api {
    folder: "directx-graphics-infrastructure",
    top_trait: Some("DirectXGraphicsInfrastructure"),
    summary: "The library dxgi.dll: adapters, outputs, formats and swap chains.",
    namespaces: &["Windows.Win32.Graphics.Dxgi", "Windows.Win32.Graphics.Dxgi.Common"],
    referenced_namespaces: &[],
    library: Some("dxgi.dll"),
    function_prefixes: &[],
    type_prefixes: &["DXGI_"],
    interface_prefixes: &["IDXGI", "IDX"],
    left_out: &[],
    abstract_bases: &["IDXGIObject", "IDXGIDeviceSubObject"],
    depends_on: &["component-object-model"],
    supertraits: &[],
    faces: &[],
};

pub const DIRECT3D_11: Api = Api {
    folder: "direct3d-11",
    top_trait: Some("Direct3D11"),
    summary: "The library d3d11.dll, and the debug layer's interfaces its objects answer.",
    namespaces: &["Windows.Win32.Graphics.Direct3D11"],
    referenced_namespaces: &[],
    library: Some("d3d11.dll"),
    function_prefixes: &["D3D11"],
    type_prefixes: &["D3D11_", "D3D_"],
    interface_prefixes: &["ID3D11", "ID3D"],
    // The shader compiler's reflection and linking (d3dcompiler_47.dll) and D3DCSX's compute helpers (d3dcsx.dll).
    left_out: &[
        "ID3D11FunctionLinkingGraph",
        "ID3D11FunctionParameterReflection",
        "ID3D11FunctionReflection",
        "ID3D11LibraryReflection",
        "ID3D11Linker",
        "ID3D11LinkingNode",
        "ID3D11Module",
        "ID3D11ModuleInstance",
        "ID3D11ShaderReflection",
        "ID3D11ShaderReflectionConstantBuffer",
        "ID3D11ShaderReflectionType",
        "ID3D11ShaderReflectionVariable",
        "ID3DX11FFT",
        "ID3DX11Scan",
        "ID3DX11SegmentedScan",
    ],
    abstract_bases: &["ID3D11DeviceChild", "ID3D11Resource", "ID3D11View", "ID3D11Asynchronous"],
    depends_on: &["component-object-model", "direct3d-common", "directx-graphics-infrastructure"],
    supertraits: &["direct3d-common", "directx-graphics-infrastructure"],
    faces: &[
        ("ID3D11Device", &["IDXGIDevice4", "ID3D11Multithread"]),
        ("ID3D11DeviceContext", &["ID3D11Multithread", "ID3DUserDefinedAnnotation"]),
        ("ID3D11Buffer", &["IDXGIResource1"]),
        ("ID3D11Texture1D", &["IDXGIResource1"]),
        ("ID3D11Texture2D", &["IDXGIResource1", "IDXGISurface2"]),
        ("ID3D11Texture3D", &["IDXGIResource1"]),
    ],
};

/// The crates written together: each API with the ones it names, lowest first.
pub const SETS: &[(&str, &[&Api])] = &[(
    "direct3d-11",
    &[&COMPONENT_OBJECT_MODEL, &DIRECT3D_COMMON, &DIRECTX_GRAPHICS_INFRASTRUCTURE, &DIRECT3D_11],
)];

pub fn by_name(name: &str) -> Option<&'static [&'static Api]> {
    SETS.iter().find(|(set, _)| *set == name).map(|(_, apis)| *apis)
}

pub fn names() -> Vec<&'static str> {
    SETS.iter().map(|(set, _)| *set).collect()
}
