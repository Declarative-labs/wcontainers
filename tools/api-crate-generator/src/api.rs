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
    /// A slice of the API: only these methods (`Interface::Method`) and exports, with the interfaces they need; empty
    /// for the whole API. The C tables keep every entry, those left out null.
    pub only: &'static [&'static str],
    /// The mapping rule's exception lines: each a C form the metadata alone does not describe, with the fact the
    /// header's documentation gives.
    pub exceptions: &'static [Exception],
}

/// How many bytes a resource's subresource holds, from the resource's description: hand-written beside the glue
/// (templates/direct3d-11-direct/lengths.rs), one rule per resource shape, each named by its suffix.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Shape {
    Buffer,
    Texture1D,
    Texture2D,
    Texture3D,
}

impl Shape {
    /// The suffix of the rule's functions in lengths.rs.
    pub fn suffix(self) -> &'static str {
        match self {
            Shape::Buffer => "buffer",
            Shape::Texture1D => "texture_1d",
            Shape::Texture2D => "texture_2d",
            Shape::Texture3D => "texture_3d",
        }
    }

    /// The object kind of a resource of this shape, by its root interface's name without the API's prefix.
    pub fn kind(self) -> &'static str {
        match self {
            Shape::Buffer => "Buffer",
            Shape::Texture1D => "Texture1D",
            Shape::Texture2D => "Texture2D",
            Shape::Texture3D => "Texture3D",
        }
    }
}

/// One exception line of the mapping rule.
#[derive(Clone, Copy, Debug)]
pub enum Exception {
    /// `method`'s `param` points at `record`s of initial data for the resource `desc` describes: one (`per_subresource`
    /// false) or one per subresource (true, `&[record]`, empty for null). Each record's `void*` field `bytes` is
    /// read-only, of the length the shape's rule gives from the description, the record's `pitch` and `slice_pitch`
    /// fields; the serving glue copies it once, the calling glue checks the caller's bytes are that long.
    InitialData {
        method: &'static str,
        param: &'static str,
        desc: &'static str,
        shape: Shape,
        per_subresource: bool,
        record: &'static str,
        bytes: &'static str,
        pitch: &'static str,
        slice_pitch: &'static str,
    },
    /// `method` (Map) lends memory in `record`'s `void*` field `field` until `until` (Unmap) is called with the same
    /// `resource` and `subresource`: the record is generic over the bytes, and the method returns it holding the top
    /// trait's `Mapped` type. The serving glue lends the C side a copy of ours, written back before `until`; the calling
    /// glue holds the system's memory as revocable bytes, revoked before `until`. The length is the resource's shape's
    /// rule from its description and the record's `row_pitch` and `depth_pitch` fields.
    Lent {
        method: &'static str,
        until: &'static str,
        resource: &'static str,
        subresource: &'static str,
        record: &'static str,
        field: &'static str,
        row_pitch: &'static str,
        depth_pitch: &'static str,
    },
    /// `method`'s `riid` and `void**` return an object of the top trait's associated type `name`, which an API built on
    /// this one binds to a kind of its own (the swap chain's buffer, which is a Direct3D texture): the glue asks that
    /// object for `riid` itself.
    BoundObject { method: &'static str, name: &'static str, kind: &'static str },
    /// `api`'s top trait's associated type `name` is bound by this API to its own kind `kind`.
    Binds { api: &'static str, name: &'static str, kind: &'static str },
    /// `function`'s handle `param` must be null: the calling glue refuses any other value, which the system would load
    /// as a module of code.
    NullHandle { function: &'static str, param: &'static str },
    /// The system's objects of `kind` and the context made with them are free threaded only under the system's
    /// multithread protection: the calling glue asks a new `kind` for `interface` and calls `method` with TRUE before
    /// handing it out, and refuses the object if it cannot.
    ProtectThreads { kind: &'static str, interface: &'static str, method: &'static str },
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
    only: &[],
    exceptions: &[],
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
    only: &[],
    exceptions: &[],
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
    only: &[],
    exceptions: &[
        // IDXGISwapChain::GetBuffer: "the interface type used to manipulate the buffer" is the API's that made the
        // swap chain; DXGI's own binding is its surface.
        Exception::BoundObject { method: "IDXGISwapChain::GetBuffer", name: "Buffer", kind: "IDXGISurface" },
    ],
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
    only: &[],
    exceptions: DIRECT3D_11_EXCEPTIONS,
};

/// Direct3D 11's exception lines, each with the page of Microsoft's documentation that states the fact.
const DIRECT3D_11_EXCEPTIONS: &[Exception] = &[
    // https://learn.microsoft.com/windows/win32/api/d3d11/ns-d3d11-d3d11_subresource_data: pSysMem holds the
    // initial data, its length from the description; one per subresource for a texture, in subresource order.
    Exception::InitialData { method: "ID3D11Device::CreateBuffer", param: "pInitialData", desc: "pDesc", shape: Shape::Buffer, per_subresource: false, record: "D3D11_SUBRESOURCE_DATA", bytes: "pSysMem", pitch: "SysMemPitch", slice_pitch: "SysMemSlicePitch" },
    Exception::InitialData { method: "ID3D11Device::CreateTexture1D", param: "pInitialData", desc: "pDesc", shape: Shape::Texture1D, per_subresource: true, record: "D3D11_SUBRESOURCE_DATA", bytes: "pSysMem", pitch: "SysMemPitch", slice_pitch: "SysMemSlicePitch" },
    Exception::InitialData { method: "ID3D11Device::CreateTexture2D", param: "pInitialData", desc: "pDesc", shape: Shape::Texture2D, per_subresource: true, record: "D3D11_SUBRESOURCE_DATA", bytes: "pSysMem", pitch: "SysMemPitch", slice_pitch: "SysMemSlicePitch" },
    Exception::InitialData { method: "ID3D11Device::CreateTexture3D", param: "pInitialData", desc: "pDesc", shape: Shape::Texture3D, per_subresource: true, record: "D3D11_SUBRESOURCE_DATA", bytes: "pSysMem", pitch: "SysMemPitch", slice_pitch: "SysMemSlicePitch" },
    // https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-map: pData is valid until
    // Unmap of the same resource and subresource.
    Exception::Lent { method: "ID3D11DeviceContext::Map", until: "ID3D11DeviceContext::Unmap", resource: "pResource", subresource: "Subresource", record: "D3D11_MAPPED_SUBRESOURCE", field: "pData", row_pitch: "RowPitch", depth_pitch: "DepthPitch" },
    // https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgiswapchain-getbuffer: a Direct3D 11 swap
    // chain's buffers are its textures.
    Exception::Binds { api: "directx-graphics-infrastructure", name: "Buffer", kind: "ID3D11Texture2D" },
    // https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-d3d11createdevice: Software is the module of a
    // software rasterizer, which the system would load and run.
    Exception::NullHandle { function: "D3D11CreateDevice", param: "Software" },
    Exception::NullHandle { function: "D3D11CreateDeviceAndSwapChain", param: "Software" },
    // https://learn.microsoft.com/windows/win32/api/d3d11_4/nf-d3d11_4-id3d11multithread-setmultithreadprotected:
    // the immediate context is free threaded only under the device's multithread protection.
    Exception::ProtectThreads { kind: "ID3D11Device", interface: "ID3D11Multithread", method: "SetMultithreadProtected" },
];

/// The spike's slice of DXGI (SPIKE.md): a surface's description, and a swap chain's Present and GetBuffer.
pub const DIRECTX_GRAPHICS_INFRASTRUCTURE_SLICE: Api = Api {
    only: &["IDXGISurface::GetDesc", "IDXGISwapChain::Present", "IDXGISwapChain::GetBuffer"],
    library: None,
    ..DIRECTX_GRAPHICS_INFRASTRUCTURE
};

/// The spike's slice of Direct3D 11 (SPIKE.md), 24 methods and exports.
pub const DIRECT3D_11_SLICE: Api = Api {
    only: &[
        "D3D11CreateDevice",
        "D3D11CreateDeviceAndSwapChain",
        "ID3D11DeviceChild::GetDevice",
        "ID3D11DeviceChild::SetPrivateDataInterface",
        "ID3D11Device::CreateBuffer",
        "ID3D11Device::CreateTexture2D",
        "ID3D11Device::CreateUnorderedAccessView",
        "ID3D11Device::CreateRenderTargetView",
        "ID3D11Device::CreateDepthStencilView",
        "ID3D11Device::CreateQuery",
        "ID3D11Device::GetImmediateContext",
        "ID3D11Resource::GetType",
        "ID3D11Buffer::GetDesc",
        "ID3D11Texture2D::GetDesc",
        "ID3D11Query::GetDesc",
        "ID3D11Asynchronous::GetDataSize",
        "ID3D11DeviceContext::Draw",
        "ID3D11DeviceContext::Map",
        "ID3D11DeviceContext::Unmap",
        "ID3D11DeviceContext::IASetVertexBuffers",
        "ID3D11DeviceContext::OMSetRenderTargetsAndUnorderedAccessViews",
        "ID3D11DeviceContext::Begin",
        "ID3D11DeviceContext::End",
        "ID3D11DeviceContext::GetData",
        // Not of the spike's 27: the calling glue turns on the system's multithread protection with it.
        "ID3D11Multithread::SetMultithreadProtected",
    ],
    faces: &[("ID3D11Device", &["ID3D11Multithread"]), ("ID3D11Texture2D", &["IDXGISurface"])],
    ..DIRECT3D_11
};

/// The crates written together: each API with the ones it names, lowest first.
pub const SETS: &[(&str, &[&Api])] = &[
    ("direct3d-11", &[&COMPONENT_OBJECT_MODEL, &DIRECT3D_COMMON, &DIRECTX_GRAPHICS_INFRASTRUCTURE, &DIRECT3D_11]),
    (
        "direct3d-11-slice",
        &[&COMPONENT_OBJECT_MODEL, &DIRECT3D_COMMON, &DIRECTX_GRAPHICS_INFRASTRUCTURE_SLICE, &DIRECT3D_11_SLICE],
    ),
];

pub fn by_name(name: &str) -> Option<&'static [&'static Api]> {
    SETS.iter().find(|(set, _)| *set == name).map(|(_, apis)| *apis)
}

pub fn names() -> Vec<&'static str> {
    SETS.iter().map(|(set, _)| *set).collect()
}
