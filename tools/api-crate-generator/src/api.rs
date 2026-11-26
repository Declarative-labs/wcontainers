//! The hand-written input: which part of the metadata each API crate is, and the mapping rule's exception lines.
//! Everything else the generator writes follows MAPPING.md from the metadata alone.

use crate::undo::Reliance;

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
    /// Every method of the whole API that src/undo.rs's search finds able to undo a fact a mark relies on, and how
    /// the calling glue holds the fact against it.
    pub pins: &'static [Pin],
}

/// One method that could undo facts the marks rely on, and how the calling glue holds them.
#[derive(Clone, Copy, Debug)]
pub struct Pin {
    /// `Interface::Method`, or an export's name.
    pub method: &'static str,
    pub relies: &'static [Reliance],
    pub hold: Hold,
    /// The harness's hostile test that tries the method through the calling glue; empty for a harmless method.
    pub test: &'static str,
}

/// How the calling glue holds a fact against a method that could undo it.
#[derive(Clone, Copy, Debug)]
pub enum Hold {
    /// The glue answers `value` (a Rust expression of the method's result) and never calls the system.
    Answers { value: &'static str, why: &'static str },
    /// The glue clears the bits `mask` of the flags `param` (a newtype or an integer) before the call.
    Clears { param: &'static str, mask: u32, why: &'static str },
    /// The glue refuses the call (E_INVALIDARG) when `when` holds, `{}` standing for the parameter `param`.
    Refuses { param: &'static str, when: &'static str, why: &'static str },
    /// The calling glue never calls the method: E_NOTIMPL, or nothing; a later milestone may hold it otherwise.
    NotCalled { why: &'static str },
    /// The search finds the method, but it cannot undo the fact.
    Harmless { why: &'static str },
}

const fn harmless(method: &'static str, relies: &'static [Reliance], why: &'static str) -> Pin {
    Pin { method, relies, hold: Hold::Harmless { why }, test: "" }
}

const fn not_called(method: &'static str, relies: &'static [Reliance], why: &'static str, test: &'static str) -> Pin {
    Pin { method, relies, hold: Hold::NotCalled { why }, test }
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
        /// The parameter that says how the memory is lent, its value that lends it for reading only (nothing is written
        /// back at the end) and its value whose contents are undefined (the shadow is cleared, not filled).
        how: &'static str,
        read_only: i32,
        discard: i32,
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
    /// `getter` (a description) is answered from what the caller passed to the method that made the object (`made_by`:
    /// the method and its parameter), recorded when the object crossed, never asked of the implementation; the length
    /// of memory Map lends is computed from the same record. `resolve` names the rule in lengths.rs that fills in what
    /// the documentation says the description reads back as (a texture's mip levels, 0 for the full chain).
    Recorded { getter: &'static str, made_by: &'static [(&'static str, &'static str)], resolve: Option<&'static str> },
    /// `getter` is answered by `rule` (lengths.rs) from the first record of these C types the object holds (a
    /// resource's kind from its description, a query's data size from its kind), never asked of the implementation;
    /// an object with no such record asks the implementation.
    Derived { getter: &'static str, sources: &'static [(&'static str, &'static str)] },
    /// `method`'s index `index` is below the `field` of the record of C type `record` the object holds (a swap chain's
    /// buffer count); past it the call fails with E_INVALIDARG, never asking the implementation.
    IndexRange { method: &'static str, index: &'static str, record: &'static str, field: &'static str },
    /// The value `value` that `method` returns is one of `list`, or of `default` when the caller passed none (a
    /// feature level); anything else is refused as E_FAIL and what was made is given back.
    OneOf { method: &'static str, value: &'static str, list: &'static str, default: &'static [i32] },
    /// `method`'s description `param` meets `rule` (lengths.rs: the ranges the header states for it); both glues refuse
    /// it with E_INVALIDARG before either side sees it.
    Checked { method: &'static str, param: &'static str, rule: &'static str },
    /// `method`'s count `count` is at most `most` (with `start`, `start + count` is), as the header's `_In_range_`
    /// states, or is the header's `keep` value, which keeps what is bound: the arrays it counts then reach the trait as
    /// `None` and are not read, nor are the parameters in `keeps`. Out of range, the glue refuses the call (E_INVALIDARG,
    /// or nothing for a method returning nothing), so that no count past the header's reads an array.
    CountRange { method: &'static str, count: &'static str, most: u32, start: Option<&'static str>, keep: Option<u32>, keeps: &'static [&'static str] },
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
    pins: &[],
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
    pins: DIRECT3D_COMMON_PINS,
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
        // https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgiswapchain-getbuffer: Buffer is below the
        // swap chain's BufferCount.
        Exception::IndexRange { method: "IDXGISwapChain::GetBuffer", index: "Buffer", record: "DXGI_SWAP_CHAIN_DESC", field: "BufferCount" },
    ],
    pins: DIRECTX_GRAPHICS_INFRASTRUCTURE_PINS,
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
    pins: DIRECT3D_11_PINS,
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
    // Unmap of the same resource and subresource; https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_map:
    // D3D11_MAP_READ (1) lends for reading only, D3D11_MAP_WRITE_DISCARD (4) leaves the previous contents undefined.
    Exception::Lent { method: "ID3D11DeviceContext::Map", until: "ID3D11DeviceContext::Unmap", resource: "pResource", subresource: "Subresource", record: "D3D11_MAPPED_SUBRESOURCE", field: "pData", row_pitch: "RowPitch", depth_pitch: "DepthPitch", how: "MapType", read_only: 1, discard: 4 },
    // https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgiswapchain-getbuffer: a Direct3D 11 swap
    // chain's buffers are its textures.
    Exception::Binds { api: "directx-graphics-infrastructure", name: "Buffer", kind: "ID3D11Texture2D" },
    // https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-d3d11createdevice: Software is the module of a
    // software rasterizer, which the system would load and run.
    Exception::NullHandle { function: "D3D11CreateDevice", param: "Software" },
    Exception::NullHandle { function: "D3D11CreateDeviceAndSwapChain", param: "Software" },
    // https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-omsetrendertargetsandunorderedaccessviews:
    // NumRTVs at most D3D11_SIMULTANEOUS_RENDER_TARGET_COUNT or D3D11_KEEP_RENDER_TARGETS_AND_DEPTH_STENCIL (which
    // keeps the depth stencil view too); UAVStartSlot + NumUAVs at most D3D11_1_UAV_SLOT_COUNT, or NumUAVs
    // D3D11_KEEP_UNORDERED_ACCESS_VIEWS.
    Exception::CountRange { method: "ID3D11DeviceContext::OMSetRenderTargetsAndUnorderedAccessViews", count: "NumRTVs", most: 8, start: None, keep: Some(0xffff_ffff), keeps: &["pDepthStencilView"] },
    Exception::CountRange { method: "ID3D11DeviceContext::OMSetRenderTargetsAndUnorderedAccessViews", count: "NumUAVs", most: 64, start: Some("UAVStartSlot"), keep: Some(0xffff_ffff), keeps: &[] },
    // https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-iasetvertexbuffers:
    // StartSlot + NumBuffers at most D3D11_IA_VERTEX_INPUT_RESOURCE_SLOT_COUNT.
    Exception::CountRange { method: "ID3D11DeviceContext::IASetVertexBuffers", count: "NumBuffers", most: 32, start: Some("StartSlot"), keep: None, keeps: &[] },
    // A resource's and a query's description are what the caller created it with (SPIKE.md, records instead of
    // answers).
    Exception::Recorded { getter: "ID3D11Buffer::GetDesc", made_by: &[("ID3D11Device::CreateBuffer", "pDesc")], resolve: None },
    Exception::Recorded { getter: "ID3D11Texture1D::GetDesc", made_by: &[("ID3D11Device::CreateTexture1D", "pDesc")], resolve: Some("resolved_texture_1d") },
    Exception::Recorded { getter: "ID3D11Texture2D::GetDesc", made_by: &[("ID3D11Device::CreateTexture2D", "pDesc")], resolve: Some("resolved_texture_2d") },
    Exception::Recorded { getter: "ID3D11Texture3D::GetDesc", made_by: &[("ID3D11Device::CreateTexture3D", "pDesc")], resolve: Some("resolved_texture_3d") },
    Exception::Recorded { getter: "ID3D11Query::GetDesc", made_by: &[("ID3D11Device::CreateQuery", "pQueryDesc")], resolve: None },
    // A swap chain's description is what it was made with.
    Exception::Recorded { getter: "IDXGISwapChain::GetDesc", made_by: &[("D3D11CreateDeviceAndSwapChain", "pSwapChainDesc")], resolve: None },
    // https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgisurface-getdesc: a texture's surface describes
    // the texture it was made as.
    Exception::Recorded { getter: "IDXGISurface::GetDesc", made_by: &[("ID3D11Device::CreateTexture2D", "pDesc")], resolve: Some("surface_of_texture_2d") },
    // https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11resource-gettype: the kind of resource;
    // https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_query: each query kind's data size.
    Exception::Derived { getter: "ID3D11Resource::GetType", sources: &[("D3D11_BUFFER_DESC", "type_of_buffer"), ("D3D11_TEXTURE1D_DESC", "type_of_texture_1d"), ("D3D11_TEXTURE2D_DESC", "type_of_texture_2d"), ("D3D11_TEXTURE3D_DESC", "type_of_texture_3d")] },
    Exception::Derived { getter: "ID3D11Asynchronous::GetDataSize", sources: &[("D3D11_QUERY_DESC", "data_size_of_query")] },
    // https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-d3d11createdevice: the feature level made is one of
    // those asked, or of the default list (11_0 down to 9_1) when none is.
    Exception::OneOf { method: "D3D11CreateDevice", value: "pFeatureLevel", list: "pFeatureLevels", default: &[0xb000, 0xa100, 0xa000, 0x9300, 0x9200, 0x9100] },
    Exception::OneOf { method: "D3D11CreateDeviceAndSwapChain", value: "pFeatureLevel", list: "pFeatureLevels", default: &[0xb000, 0xa100, 0xa000, 0x9300, 0x9200, 0x9100] },
    // The ranges d3d11.h states for a description: a resource of at most the largest resource's bytes, sides and
    // slices within D3D11_REQ_TEXTURE*_DIMENSION, mips within the full chain, declared usages and flags; a swap
    // chain's buffers within the same sides (SPIKE.md: the defect the fuzzing found).
    Exception::Checked { method: "ID3D11Device::CreateBuffer", param: "pDesc", rule: "checked_buffer" },
    Exception::Checked { method: "ID3D11Device::CreateTexture1D", param: "pDesc", rule: "checked_texture_1d" },
    Exception::Checked { method: "ID3D11Device::CreateTexture2D", param: "pDesc", rule: "checked_texture_2d" },
    Exception::Checked { method: "ID3D11Device::CreateTexture3D", param: "pDesc", rule: "checked_texture_3d" },
    Exception::Checked { method: "D3D11CreateDeviceAndSwapChain", param: "pSwapChainDesc", rule: "checked_swap_chain" },
    // https://learn.microsoft.com/windows/win32/api/d3dcommon/ne-d3dcommon-d3d_feature_level: a list of feature levels
    // names each at most once, and d3dcommon.h declares fewer than 16 (the spike's bound, LEVELS).
    Exception::CountRange { method: "D3D11CreateDevice", count: "FeatureLevels", most: 16, start: None, keep: None, keeps: &[] },
    Exception::CountRange { method: "D3D11CreateDeviceAndSwapChain", count: "FeatureLevels", most: 16, start: None, keep: None, keeps: &[] },
    // https://learn.microsoft.com/windows/win32/api/d3d11_4/nn-d3d11_4-id3d11multithread: the immediate context is
    // free threaded only under its multithread protection, which the context answers by QueryInterface.
    Exception::ProtectThreads { kind: "ID3D11DeviceContext", interface: "ID3D11Multithread", method: "SetMultithreadProtected" },
];

use Reliance::{FreeThreaded, LentUntilEnd, LiveObject, NoCallback, ReturnsNormally};

const PRIVATE_DATA: &str = "the glue keeps nothing in private data: the bytes cross copied, and an interface stored is held by the system's own reference";
const EVICTION: &str = "residency on the GPU: an object or memory lent to the CPU stays live and mapped";
const BREAK: &str = "a break raises a breakpoint exception inside the call, through our frames";
const BREAK_READ: &str = "reads a setting, which the glue never turns on";
const DISCARDED_COUNT: &str = "a count of discarded messages, no object or memory";
const MESSAGE_ID: &str = "the search's flag test finds D3D11_MESSAGE_ID, whose values name messages about single-threaded use; an identifier, no flag";
const DEBUG_LAYER: &str = "a debug layer counts references as COM does and only lengthens lifetimes: what the glue holds stays live";

/// d3dcommon's pins.
const DIRECT3D_COMMON_PINS: &[Pin] = &[
    not_called("ID3DDestructionNotifier::RegisterDestructionCallback", &[NoCallback], "a function pointer the system would call: no row maps one, so the glue writes no way to hand one over", "destruction_callbacks_are_never_registered"),
    harmless("ID3DDestructionNotifier::UnregisterDestructionCallback", &[NoCallback], "removes a callback, of which the glue registers none"),
];

/// DXGI's pins.
const DIRECTX_GRAPHICS_INFRASTRUCTURE_PINS: &[Pin] = &[
    harmless("IDXGIObject::SetPrivateData", &[LiveObject], PRIVATE_DATA),
    harmless("IDXGIObject::SetPrivateDataInterface", &[LiveObject], PRIVATE_DATA),
    harmless("IDXGIObject::GetPrivateData", &[LiveObject], PRIVATE_DATA),
    not_called("IDXGIDevice2::OfferResources", &[LiveObject, LentUntilEnd], "an offered resource's memory may be discarded, under memory a view may hold", "offered_resources_are_never_offered"),
    not_called("IDXGIDevice2::ReclaimResources", &[LentUntilEnd], "pairs with OfferResources, which is never called", "offered_resources_are_never_offered"),
    not_called("IDXGIDevice4::OfferResources1", &[LiveObject, LentUntilEnd], "as OfferResources", "offered_resources_are_never_offered"),
    not_called("IDXGIDevice4::ReclaimResources1", &[LentUntilEnd], "as ReclaimResources", "offered_resources_are_never_offered"),
    harmless("IDXGIDevice3::Trim", &[LiveObject, LentUntilEnd], "frees the driver's own allocations, never an object referenced or memory mapped"),
    harmless("IDXGIInfoQueue::GetNumMessagesDiscardedByMessageCountLimit", &[LiveObject, LentUntilEnd], DISCARDED_COUNT),
    Pin { method: "IDXGIInfoQueue::SetBreakOnCategory", relies: &[ReturnsNormally], hold: Hold::Refuses { param: "bEnable", when: "{}", why: BREAK }, test: "breaks_are_never_turned_on" },
    Pin { method: "IDXGIInfoQueue::SetBreakOnSeverity", relies: &[ReturnsNormally], hold: Hold::Refuses { param: "bEnable", when: "{}", why: BREAK }, test: "breaks_are_never_turned_on" },
    Pin { method: "IDXGIInfoQueue::SetBreakOnID", relies: &[ReturnsNormally], hold: Hold::Refuses { param: "bEnable", when: "{}", why: BREAK }, test: "breaks_are_never_turned_on" },
    harmless("IDXGIInfoQueue::GetBreakOnCategory", &[ReturnsNormally], BREAK_READ),
    harmless("IDXGIInfoQueue::GetBreakOnSeverity", &[ReturnsNormally], BREAK_READ),
    harmless("IDXGIInfoQueue::GetBreakOnID", &[ReturnsNormally], BREAK_READ),
    harmless("IDXGIKeyedMutex::ReleaseSync", &[LiveObject], "releases a keyed mutex shared between devices, no reference"),
    harmless("IDXGIOutput::ReleaseOwnership", &[LiveObject], "gives up an output's exclusive ownership, no reference"),
    not_called("IDXGIOutputDuplication::MapDesktopSurface", &[LentUntilEnd], "lends the desktop's memory, for which no exception line says until when", "desktop_surfaces_are_never_mapped"),
    not_called("IDXGIOutputDuplication::UnMapDesktopSurface", &[LentUntilEnd], "pairs with MapDesktopSurface, which is never called", "desktop_surfaces_are_never_mapped"),
    harmless("IDXGIOutputDuplication::ReleaseFrame", &[LiveObject], "ends a frame's use of the desktop image; the image object stays live while referenced, and is never mapped"),
    harmless("IDXGIDebug::ReportLiveObjects", &[LiveObject], "reports live objects to the debugger; it changes no count"),
    harmless("IDXGIResource::SetEvictionPriority", &[LiveObject, LentUntilEnd], EVICTION),
    harmless("IDXGIResource::GetEvictionPriority", &[LiveObject, LentUntilEnd], EVICTION),
    not_called("IDXGISurface::Map", &[LentUntilEnd], "would map a texture's memory beside the context's Map, and its Unmap end memory a view holds", "surfaces_are_never_mapped_through_dxgi"),
    not_called("IDXGISurface::Unmap", &[LentUntilEnd], "could unmap a subresource the context lent, behind its view", "surfaces_are_never_mapped_through_dxgi"),
    not_called("IDXGISurface1::GetDC", &[LentUntilEnd], "locks a surface for GDI, beside memory a view may hold", "surfaces_are_never_mapped_through_dxgi"),
    not_called("IDXGISurface1::ReleaseDC", &[LiveObject, LentUntilEnd], "pairs with GetDC, which is never called", "surfaces_are_never_mapped_through_dxgi"),
    harmless("IDXGISwapChain::ResizeBuffers", &[LentUntilEnd], "fails while any reference to a buffer is held, and lent memory holds its resource"),
    harmless("IDXGISwapChain::ResizeTarget", &[LentUntilEnd], "resizes the window, no buffer"),
    harmless("IDXGISwapChain3::ResizeBuffers1", &[LentUntilEnd], "as ResizeBuffers"),
    harmless("CreateDXGIFactory2", &[LiveObject], DEBUG_LAYER),
];

/// Direct3D 11's pins.
const DIRECT3D_11_PINS: &[Pin] = &[
    // https://learn.microsoft.com/windows/win32/api/d3d11_4/nn-d3d11_4-id3d11multithread: the immediate context is
    // free threaded only while protected.
    Pin { method: "ID3D11Multithread::SetMultithreadProtected", relies: &[FreeThreaded], hold: Hold::Answers { value: "true", why: "the glue turned protection on when the context crossed and keeps it on: it was on before the call, which is what the method returns" }, test: "protection_stays_on_through_the_context_and_the_multithread_object" },
    harmless("ID3D11Multithread::GetMultithreadProtected", &[FreeThreaded], "reads the protection, which the glue keeps on"),
    not_called("ID3D11Multithread::Enter", &[FreeThreaded], "a Leave without its Enter on the same thread breaks the system's lock; glued with a per-thread count when the whole API is (AC3)", "the_device_lock_is_never_taken_or_left"),
    not_called("ID3D11Multithread::Leave", &[FreeThreaded], "as Enter", "the_device_lock_is_never_taken_or_left"),
    not_called("ID3D11Device::CreateDeferredContext", &[FreeThreaded], "a deferred context is not free threaded, and its kind's type is Sync", "deferred_contexts_are_never_made"),
    not_called("ID3D11Device1::CreateDeferredContext1", &[FreeThreaded], "as CreateDeferredContext", "deferred_contexts_are_never_made"),
    not_called("ID3D11Device2::CreateDeferredContext2", &[FreeThreaded], "as CreateDeferredContext", "deferred_contexts_are_never_made"),
    not_called("ID3D11Device3::CreateDeferredContext3", &[FreeThreaded], "as CreateDeferredContext", "deferred_contexts_are_never_made"),
    // https://learn.microsoft.com/windows/win32/api/d3d11_1/ne-d3d11_1-d3d11_1_create_device_context_state_flag.
    Pin { method: "ID3D11Device1::CreateDeviceContextState", relies: &[FreeThreaded], hold: Hold::Clears { param: "Flags", mask: 0x1, why: "D3D11_1_CREATE_DEVICE_CONTEXT_STATE_SINGLETHREADED would make the context it is swapped into single threaded" }, test: "a_context_state_is_asked_free_threaded" },
    harmless("ID3D11DeviceContext1::SwapDeviceContextState", &[FreeThreaded], "a state made through the glue is never single threaded (CreateDeviceContextState's flag is cleared)"),
    // https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_create_device_flag.
    Pin { method: "D3D11CreateDevice", relies: &[FreeThreaded], hold: Hold::Clears { param: "Flags", mask: 0x1, why: "D3D11_CREATE_DEVICE_SINGLETHREADED makes the device and its context single threaded; a free-threaded device serves a single-threaded caller the same" }, test: "a_single_threaded_device_is_asked_free_threaded" },
    Pin { method: "D3D11CreateDeviceAndSwapChain", relies: &[FreeThreaded], hold: Hold::Clears { param: "Flags", mask: 0x1, why: "as D3D11CreateDevice" }, test: "a_single_threaded_device_is_asked_free_threaded" },
    harmless("D3D11CreateDevice", &[LiveObject], DEBUG_LAYER),
    harmless("D3D11CreateDeviceAndSwapChain", &[LiveObject], DEBUG_LAYER),
    // https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11device-setexceptionmode.
    Pin { method: "ID3D11Device::SetExceptionMode", relies: &[ReturnsNormally], hold: Hold::Refuses { param: "RaiseFlags", when: "{} != 0", why: "D3D11_RAISE_FLAG_DRIVER_INTERNAL_ERROR raises an exception inside a call, through our frames" }, test: "exceptions_are_never_turned_on" },
    harmless("ID3D11Device::GetExceptionMode", &[ReturnsNormally], BREAK_READ),
    Pin { method: "ID3D11InfoQueue::SetBreakOnCategory", relies: &[ReturnsNormally], hold: Hold::Refuses { param: "bEnable", when: "{}", why: BREAK }, test: "breaks_are_never_turned_on" },
    Pin { method: "ID3D11InfoQueue::SetBreakOnSeverity", relies: &[ReturnsNormally], hold: Hold::Refuses { param: "bEnable", when: "{}", why: BREAK }, test: "breaks_are_never_turned_on" },
    Pin { method: "ID3D11InfoQueue::SetBreakOnID", relies: &[ReturnsNormally], hold: Hold::Refuses { param: "bEnable", when: "{}", why: BREAK }, test: "breaks_are_never_turned_on" },
    harmless("ID3D11InfoQueue::SetBreakOnID", &[FreeThreaded], MESSAGE_ID),
    harmless("ID3D11InfoQueue::GetBreakOnID", &[FreeThreaded], MESSAGE_ID),
    harmless("ID3D11InfoQueue::AddMessage", &[FreeThreaded], MESSAGE_ID),
    harmless("ID3D11InfoQueue::GetBreakOnCategory", &[ReturnsNormally], BREAK_READ),
    harmless("ID3D11InfoQueue::GetBreakOnSeverity", &[ReturnsNormally], BREAK_READ),
    harmless("ID3D11InfoQueue::GetBreakOnID", &[ReturnsNormally], BREAK_READ),
    harmless("ID3D11InfoQueue::GetNumMessagesDiscardedByMessageCountLimit", &[LiveObject, LentUntilEnd], DISCARDED_COUNT),
    harmless("ID3D11DeviceContext::Map", &[LentUntilEnd], "the exception line's own method: a subresource mapped through the glue is not mapped again before its Unmap"),
    harmless("ID3D11DeviceContext::Unmap", &[LentUntilEnd], "the exception line's end: the glue revokes every view of the memory before calling it"),
    not_called("ID3D11DeviceContext1::DiscardResource", &[LiveObject, LentUntilEnd], "discards a resource's contents, under memory a view may hold; glued refused while lent when the whole API is (AC3)", "resources_are_never_discarded"),
    not_called("ID3D11DeviceContext1::DiscardView", &[LiveObject, LentUntilEnd], "as DiscardResource", "resources_are_never_discarded"),
    not_called("ID3D11DeviceContext1::DiscardView1", &[LiveObject, LentUntilEnd], "as DiscardResource", "resources_are_never_discarded"),
    harmless("ID3D11DeviceContext2::UpdateTileMappings", &[LentUntilEnd], "tile mappings bind GPU memory to tiled resources, which are never mapped for the CPU"),
    harmless("ID3D11DeviceContext2::CopyTileMappings", &[LentUntilEnd], "as UpdateTileMappings"),
    harmless("ID3D11DeviceContext2::ResizeTilePool", &[LentUntilEnd], "a tile pool is never mapped for the CPU"),
    not_called("ID3D11SwitchToRef::SetUseRef", &[LentUntilEnd], "swaps the device's rasterizer under every object and mapping", "the_reference_rasterizer_is_never_switched_to"),
    harmless("ID3D11SwitchToRef::GetUseRef", &[LentUntilEnd], BREAK_READ),
    not_called("ID3D11VideoContext::GetDecoderBuffer", &[LentUntilEnd], "lends memory until ReleaseDecoderBuffer, for which no exception line is written yet", "decoder_buffers_are_never_lent"),
    not_called("ID3D11VideoContext::ReleaseDecoderBuffer", &[LiveObject, LentUntilEnd], "pairs with GetDecoderBuffer, which is never called", "decoder_buffers_are_never_lent"),
    harmless("ID3D11VideoContext::SubmitDecoderBuffers", &[LentUntilEnd], "submits buffers already given back; the glue lends none"),
    harmless("ID3D11VideoContext1::SubmitDecoderBuffers1", &[LentUntilEnd], "as SubmitDecoderBuffers"),
    harmless("ID3D11VideoContext3::SubmitDecoderBuffers2", &[LentUntilEnd], "as SubmitDecoderBuffers"),
    harmless("ID3D11Resource::SetEvictionPriority", &[LiveObject, LentUntilEnd], EVICTION),
    harmless("ID3D11Resource::GetEvictionPriority", &[LiveObject, LentUntilEnd], EVICTION),
    harmless("ID3D11Device::GetPrivateData", &[LiveObject], PRIVATE_DATA),
    harmless("ID3D11Device::SetPrivateData", &[LiveObject], PRIVATE_DATA),
    harmless("ID3D11Device::SetPrivateDataInterface", &[LiveObject], PRIVATE_DATA),
    harmless("ID3D11DeviceChild::GetPrivateData", &[LiveObject], PRIVATE_DATA),
    harmless("ID3D11DeviceChild::SetPrivateData", &[LiveObject], PRIVATE_DATA),
    harmless("ID3D11DeviceChild::SetPrivateDataInterface", &[LiveObject], PRIVATE_DATA),
    harmless("ID3D11VideoDevice::SetPrivateData", &[LiveObject], PRIVATE_DATA),
    harmless("ID3D11VideoDevice::SetPrivateDataInterface", &[LiveObject], PRIVATE_DATA),
    harmless("ID3D11VideoDevice1::GetCryptoSessionPrivateDataSize", &[LiveObject], "a size, no data"),
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
    faces: &[("ID3D11DeviceContext", &["ID3D11Multithread"]), ("ID3D11Texture2D", &["IDXGISurface"])],
    ..DIRECT3D_11
};

/// The crates written together: each API with the ones it names, lowest first.
pub const SETS: &[(&str, &[&Api])] = &[
    ("direct3d-11", &[&COMPONENT_OBJECT_MODEL, &DIRECT3D_COMMON, &DIRECTX_GRAPHICS_INFRASTRUCTURE, &DIRECT3D_11]),
    // The same APIs with both glues for every method (AC3), kept apart from the trait layer until they build.
    ("direct3d-11-glue", &[&COMPONENT_OBJECT_MODEL, &DIRECT3D_COMMON, &DIRECTX_GRAPHICS_INFRASTRUCTURE, &DIRECT3D_11]),
    (
        "direct3d-11-slice",
        &[&COMPONENT_OBJECT_MODEL, &DIRECT3D_COMMON, &DIRECTX_GRAPHICS_INFRASTRUCTURE_SLICE, &DIRECT3D_11_SLICE],
    ),
];

/// The whole API of a folder, of which a slice is a part.
pub fn whole(folder: &str) -> Option<&'static Api> {
    by_name("direct3d-11")?.iter().copied().find(|api| api.folder == folder)
}

/// Whether a set's direct crates are written: a slice's, and the whole APIs' glue set.
pub fn writes_glue(name: &str) -> bool {
    name != "direct3d-11"
}

pub fn by_name(name: &str) -> Option<&'static [&'static Api]> {
    SETS.iter().find(|(set, _)| *set == name).map(|(_, apis)| *apis)
}

pub fn names() -> Vec<&'static str> {
    SETS.iter().map(|(set, _)| *set).collect()
}
