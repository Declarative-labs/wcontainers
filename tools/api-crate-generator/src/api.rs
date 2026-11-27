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
    /// Interfaces every object kind of the API answers through QueryInterface, beside its own faces.
    pub every_kind: &'static [&'static str],
    /// A slice of the API: only these methods (`Interface::Method`) and exports, with the interfaces they need; empty
    /// for the whole API. The C tables keep every entry, those left out null.
    pub only: &'static [&'static str],
    /// The mapping rule's exception lines: each a C form the metadata alone does not describe, with the fact the
    /// header's documentation gives.
    pub exceptions: &'static [Exception],
    /// The rows for exact methods (src/kinds.rs): a method whose effect could undo a fact a mark relies on, or one
    /// with a parameter of a kind the system trusts and acts on, and how the calling glue treats it.
    pub pins: &'static [Pin],
    /// How every value of each of the API's enumerations is judged; an enumeration no line judges fails the generator.
    pub judged: &'static [Judged],
    /// Values of the API's enumerations that the calling glue clears or refuses wherever a parameter passes them.
    pub specials: &'static [SpecialValue],
    /// The effects of every method of each of the API's interfaces and exports, judged an interface at a time: what
    /// none of its methods (but the pins) can do to a fact the marks rely on. An interface or export of the set no line
    /// judges fails the generator.
    pub effects: &'static [Effects],
}

impl Api {
    /// The interfaces of other kinds an object of the kind rooted at `root` answers by QueryInterface: its own faces,
    /// then those every kind of the API answers.
    pub fn faces_of(&self, root: &str) -> Vec<&'static str> {
        let own = self.faces.iter().filter(|(r, _)| *r == root).flat_map(|(_, faces)| faces.iter().copied());
        own.chain(self.every_kind.iter().copied().filter(|face| *face != root)).collect()
    }
}

/// One row for an exact method, and how the calling glue treats it.
#[derive(Clone, Copy, Debug)]
pub struct Pin {
    /// `Interface::Method`, or an export's name.
    pub method: &'static str,
    /// The facts the marks rely on that the row holds (none for a row about a parameter's kind alone).
    pub relies: &'static [Reliance],
    pub hold: Hold,
    /// The hostile test that tries the method through the calling glue.
    pub test: &'static str,
}

/// How the calling glue treats the method of a row.
#[derive(Clone, Copy, Debug)]
pub enum Hold {
    /// The glue answers `value` (a Rust expression of the method's result) and never calls the system.
    Answers { value: &'static str, why: &'static str },
    /// The glue clears the bits `mask` of the number `param` (one the metadata links to no enumeration) before the call.
    Clears { param: &'static str, mask: u32, why: &'static str },
    /// The glue refuses the call (E_INVALIDARG) when `when` holds, `{}` standing for the parameter `param`.
    Refuses { param: &'static str, when: &'static str, why: &'static str },
    /// The calling glue never calls the method: E_NOTIMPL, or nothing; a later milestone may hold it otherwise.
    NotCalled { why: &'static str },
    /// `param`, of a kind the system trusts and acts on, passes only as null (0); the glue refuses any other value.
    NullOnly { param: &'static str, why: &'static str },
}

const fn not_called(method: &'static str, relies: &'static [Reliance], why: &'static str, test: &'static str) -> Pin {
    Pin { method, relies, hold: Hold::NotCalled { why }, test }
}

/// Which values of an enumeration pass as plain numbers (src/kinds.rs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Values {
    /// Every value of the integer, declared or not, is plain: the system only compares it or checks it against the
    /// values it knows.
    Any,
    /// Only the declared values pass (for a flag set, only the declared bits); the calling glue refuses any other.
    Declared,
}

/// One judgment of every value of some enumerations, with its reason.
#[derive(Clone, Copy, Debug)]
pub struct Judged {
    pub enumerations: &'static [&'static str],
    pub values: Values,
    pub why: &'static str,
}

/// What the calling glue does with one value of an enumeration a parameter passes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Special {
    /// The value's bits are cleared before the call.
    Clear,
    /// The call is refused (E_INVALIDARG) when the value is passed (for a flag set, when any of its bits is set).
    Refuse,
}

/// One value of an enumeration the calling glue clears or refuses, with its reason and hostile test.
#[derive(Clone, Copy, Debug)]
pub struct SpecialValue {
    pub enumeration: &'static str,
    pub value: &'static str,
    pub rule: Special,
    pub why: &'static str,
    pub test: &'static str,
}

/// The effects of every method of some interfaces (or exports) judged together: none of them, except a method a pin
/// names, changes a fact a mark relies on.
#[derive(Clone, Copy, Debug)]
pub struct Effects {
    pub interfaces: &'static [&'static str],
    pub why: &'static str,
}

/// What a handle typedef of the metadata is (src/kinds.rs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandleKind {
    /// A handle the system's kernel looks up in this process's tables and checks before acting on it.
    Kernel,
    /// A handle or address the system trusts and acts on as it is (a module, a pointer).
    Trusted,
    /// A plain number with a typedef of its own (a color, a status of another kind).
    Number,
}

/// Every typedef of one value the metadata declares that the set's methods or records name, and what it is; one no line
/// names fails the generator.
pub const HANDLES: &[(&str, HandleKind, &str)] = &[
    ("HANDLE", HandleKind::Kernel, "an object handle the kernel looks up in this process's handle table, checking the object's type and the access granted"),
    ("HWND", HandleKind::Kernel, "a window handle win32k looks up and checks; the system only sends the window messages any program may send it"),
    ("HDC", HandleKind::Kernel, "a device context handle the kernel's GDI table looks up and checks"),
    ("HMONITOR", HandleKind::Kernel, "a monitor handle win32k looks up and checks"),
    ("HMODULE", HandleKind::Trusted, "the address of a loaded module, whose code the system would find and run"),
    ("HINSTANCE", HandleKind::Trusted, "the address of a loaded module, whose code the system would find and run"),
    ("LPARAM", HandleKind::Trusted, "a message's word, which a window procedure may read as an address"),
    ("WPARAM", HandleKind::Trusted, "a message's word, which a window procedure may read as an address"),
    ("PSID", HandleKind::Trusted, "the address of a security identifier, which the system reads"),
    ("PSECURITY_DESCRIPTOR", HandleKind::Trusted, "the address of a security descriptor, which the system reads"),
    ("NTSTATUS", HandleKind::Number, "a status code"),
    ("COLORREF", HandleKind::Number, "a color as a number"),
];

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
    every_kind: &[],
    only: &[],
    exceptions: &[],
    pins: &[],
    judged: &[],
    specials: &[],
    effects: &[],
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
    every_kind: &[],
    only: &[],
    exceptions: &[],
    pins: DIRECT3D_COMMON_PINS,
    judged: DIRECT3D_COMMON_JUDGED,
    specials: &[],
    effects: DIRECT3D_COMMON_EFFECTS,
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
    every_kind: &[],
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
    judged: DIRECTX_GRAPHICS_INFRASTRUCTURE_JUDGED,
    specials: DIRECTX_GRAPHICS_INFRASTRUCTURE_SPECIALS,
    effects: DIRECTX_GRAPHICS_INFRASTRUCTURE_EFFECTS,
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
    // Every Direct3D object answers d3dcommon's ID3DDestructionNotifier: "To acquire an instance of this interface,
    // call QueryInterface on a Direct3D object with the IID of ID3DDestructionNotifier"
    // (https://learn.microsoft.com/windows/win32/api/d3dcommon/nn-d3dcommon-id3ddestructionotifier, READ 2026-11-27).
    every_kind: &["ID3DDestructionNotifier"],
    only: &[],
    exceptions: DIRECT3D_11_EXCEPTIONS,
    pins: DIRECT3D_11_PINS,
    judged: DIRECT3D_11_JUDGED,
    specials: DIRECT3D_11_SPECIALS,
    effects: DIRECT3D_11_EFFECTS,
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

use Reliance::{FreeThreaded, LentUntilEnd, LiveObject, ReturnsNormally};

const BREAK: &str = "a break raises a breakpoint exception inside the call, through our frames";

/// d3dcommon's rows: none. Its one callback (RegisterDestructionCallback's function pointer) is a kind the system
/// trusts, refused by default.
const DIRECT3D_COMMON_PINS: &[Pin] = &[];

/// DXGI's rows.
const DIRECTX_GRAPHICS_INFRASTRUCTURE_PINS: &[Pin] = &[
    not_called("IDXGIDevice2::OfferResources", &[LiveObject, LentUntilEnd], "an offered resource's memory may be discarded, under memory a view may hold", "offered_resources_are_never_offered"),
    not_called("IDXGIDevice2::ReclaimResources", &[LentUntilEnd], "pairs with OfferResources, which is never called", "offered_resources_are_never_offered"),
    not_called("IDXGIDevice4::OfferResources1", &[LiveObject, LentUntilEnd], "as OfferResources", "offered_resources_are_never_offered"),
    not_called("IDXGIDevice4::ReclaimResources1", &[LentUntilEnd], "as ReclaimResources", "offered_resources_are_never_offered"),
    Pin { method: "IDXGIInfoQueue::SetBreakOnCategory", relies: &[ReturnsNormally], hold: Hold::Refuses { param: "bEnable", when: "{}", why: BREAK }, test: "breaks_are_never_turned_on" },
    Pin { method: "IDXGIInfoQueue::SetBreakOnSeverity", relies: &[ReturnsNormally], hold: Hold::Refuses { param: "bEnable", when: "{}", why: BREAK }, test: "breaks_are_never_turned_on" },
    Pin { method: "IDXGIInfoQueue::SetBreakOnID", relies: &[ReturnsNormally], hold: Hold::Refuses { param: "bEnable", when: "{}", why: BREAK }, test: "breaks_are_never_turned_on" },
    not_called("IDXGIOutputDuplication::MapDesktopSurface", &[LentUntilEnd], "lends the desktop's memory, for which no exception line says until when", "desktop_surfaces_are_never_mapped"),
    not_called("IDXGIOutputDuplication::UnMapDesktopSurface", &[LentUntilEnd], "pairs with MapDesktopSurface, which is never called", "desktop_surfaces_are_never_mapped"),
    not_called("IDXGISurface::Map", &[LentUntilEnd], "would map a texture's memory beside the context's Map, and its Unmap end memory a view holds", "surfaces_are_never_mapped_through_dxgi"),
    not_called("IDXGISurface::Unmap", &[LentUntilEnd], "could unmap a subresource the context lent, behind its view", "surfaces_are_never_mapped_through_dxgi"),
    not_called("IDXGISurface1::GetDC", &[LentUntilEnd], "locks a surface for GDI, beside memory a view may hold", "surfaces_are_never_mapped_through_dxgi"),
    not_called("IDXGISurface1::ReleaseDC", &[LiveObject, LentUntilEnd], "pairs with GetDC, which is never called", "surfaces_are_never_mapped_through_dxgi"),
];

/// Direct3D 11's rows.
const DIRECT3D_11_PINS: &[Pin] = &[
    // https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-d3d11createdevice: Software is the module of a
    // software rasterizer, which the system would load and run.
    Pin { method: "D3D11CreateDevice", relies: &[], hold: Hold::NullOnly { param: "Software", why: "a module of a software rasterizer, which the system would load and run; null asks for none" }, test: "a_module_is_never_handed_to_the_system" },
    Pin { method: "D3D11CreateDeviceAndSwapChain", relies: &[], hold: Hold::NullOnly { param: "Software", why: "as D3D11CreateDevice" }, test: "a_module_is_never_handed_to_the_system" },
    // https://learn.microsoft.com/windows/win32/api/d3d11_4/nn-d3d11_4-id3d11multithread: the immediate context is
    // free threaded only while protected.
    Pin { method: "ID3D11Multithread::SetMultithreadProtected", relies: &[FreeThreaded], hold: Hold::Answers { value: "true", why: "the glue turned protection on when the context crossed and keeps it on: it was on before the call, which is what the method returns" }, test: "protection_stays_on_through_the_context_and_the_multithread_object" },
    not_called("ID3D11Multithread::Enter", &[FreeThreaded], "a Leave without its Enter on the same thread breaks the system's lock; glued with a per-thread count later", "the_device_lock_is_never_taken_or_left"),
    not_called("ID3D11Multithread::Leave", &[FreeThreaded], "as Enter", "the_device_lock_is_never_taken_or_left"),
    not_called("ID3D11Device::CreateDeferredContext", &[FreeThreaded], "a deferred context is not free threaded, and its kind's type is Sync", "deferred_contexts_are_never_made"),
    not_called("ID3D11Device1::CreateDeferredContext1", &[FreeThreaded], "as CreateDeferredContext", "deferred_contexts_are_never_made"),
    not_called("ID3D11Device2::CreateDeferredContext2", &[FreeThreaded], "as CreateDeferredContext", "deferred_contexts_are_never_made"),
    not_called("ID3D11Device3::CreateDeferredContext3", &[FreeThreaded], "as CreateDeferredContext", "deferred_contexts_are_never_made"),
    // https://learn.microsoft.com/windows/win32/api/d3d11_1/ne-d3d11_1-d3d11_1_create_device_context_state_flag.
    Pin { method: "ID3D11Device1::CreateDeviceContextState", relies: &[FreeThreaded], hold: Hold::Clears { param: "Flags", mask: 0x1, why: "D3D11_1_CREATE_DEVICE_CONTEXT_STATE_SINGLETHREADED would make the context it is swapped into single threaded" }, test: "a_context_state_is_asked_free_threaded" },
    // https://learn.microsoft.com/windows/win32/api/d3d11_1/ne-d3d11_1-d3d11_copy_flags.
    Pin { method: "ID3D11DeviceContext1::CopySubresourceRegion1", relies: &[LentUntilEnd], hold: Hold::Refuses { param: "CopyFlags", when: "{} & 0x2 != 0", why: "D3D11_COPY_DISCARD lets the system drop the destination's memory, under memory a view may hold, as DiscardResource (never called)" }, test: "a_discarding_copy_is_refused" },
    // https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11device-setexceptionmode.
    Pin { method: "ID3D11Device::SetExceptionMode", relies: &[ReturnsNormally], hold: Hold::Refuses { param: "RaiseFlags", when: "{} != 0", why: "D3D11_RAISE_FLAG_DRIVER_INTERNAL_ERROR raises an exception inside a call, through our frames" }, test: "exceptions_are_never_turned_on" },
    Pin { method: "ID3D11InfoQueue::SetBreakOnCategory", relies: &[ReturnsNormally], hold: Hold::Refuses { param: "bEnable", when: "{}", why: BREAK }, test: "breaks_are_never_turned_on" },
    Pin { method: "ID3D11InfoQueue::SetBreakOnSeverity", relies: &[ReturnsNormally], hold: Hold::Refuses { param: "bEnable", when: "{}", why: BREAK }, test: "breaks_are_never_turned_on" },
    Pin { method: "ID3D11InfoQueue::SetBreakOnID", relies: &[ReturnsNormally], hold: Hold::Refuses { param: "bEnable", when: "{}", why: BREAK }, test: "breaks_are_never_turned_on" },
    not_called("ID3D11DeviceContext1::DiscardResource", &[LiveObject, LentUntilEnd], "discards a resource's contents, under memory a view may hold; glued refused while lent later", "resources_are_never_discarded"),
    not_called("ID3D11DeviceContext1::DiscardView", &[LiveObject, LentUntilEnd], "as DiscardResource", "resources_are_never_discarded"),
    not_called("ID3D11DeviceContext1::DiscardView1", &[LiveObject, LentUntilEnd], "as DiscardResource", "resources_are_never_discarded"),
    not_called("ID3D11SwitchToRef::SetUseRef", &[LentUntilEnd], "swaps the device's rasterizer under every object and mapping", "the_reference_rasterizer_is_never_switched_to"),
    not_called("ID3D11VideoContext::GetDecoderBuffer", &[LentUntilEnd], "lends memory until ReleaseDecoderBuffer, for which no exception line is written yet", "decoder_buffers_are_never_lent"),
    not_called("ID3D11VideoContext::ReleaseDecoderBuffer", &[LiveObject, LentUntilEnd], "pairs with GetDecoderBuffer, which is never called", "decoder_buffers_are_never_lent"),
];

/// A selector or flag set the system compares with the values it knows, refusing any other with an error.
const CHECKED_BY_SYSTEM: &str = "a selector or flag set the system compares with the values it knows and refuses any other with an error; no value names memory, code, a thread or an exception";
/// An identifier of a message or a category, only compared and stored.
const MESSAGE_KEYS: &str = "an identifier of a debug message, its category or severity: the system only compares and stores it (a break on one is a row of its own)";

/// The judgments of d3dcommon's enumerations.
const DIRECT3D_COMMON_JUDGED: &[Judged] = &[
    Judged { enumerations: &["D3D_DRIVER_TYPE", "D3D_FEATURE_LEVEL", "D3D_PRIMITIVE_TOPOLOGY", "D3D_SRV_DIMENSION"], values: Values::Any, why: CHECKED_BY_SYSTEM },
];

/// The judgments of DXGI's enumerations, and of its numbers named as flags that the metadata links to none.
const DIRECTX_GRAPHICS_INFRASTRUCTURE_JUDGED: &[Judged] = &[
    Judged {
        enumerations: &[
            "DXGI_ADAPTER_FLAG", "DXGI_ADAPTER_FLAG3", "DXGI_ALPHA_MODE", "DXGI_COLOR_SPACE_TYPE", "DXGI_COMPUTE_PREEMPTION_GRANULARITY", "DXGI_DEBUG_RLO_FLAGS",
            "DXGI_ENUM_MODES", "DXGI_FEATURE", "DXGI_FORMAT", "DXGI_FRAME_PRESENTATION_MODE", "DXGI_GPU_PREFERENCE", "DXGI_GRAPHICS_PREEMPTION_GRANULARITY",
            "DXGI_HDR_METADATA_TYPE", "DXGI_MEMORY_SEGMENT_GROUP", "DXGI_MODE_ROTATION", "DXGI_MODE_SCALING", "DXGI_MODE_SCANLINE_ORDER",
            "DXGI_MULTIPLANE_OVERLAY_YCbCr_FLAGS", "DXGI_OFFER_RESOURCE_FLAGS", "DXGI_OFFER_RESOURCE_PRIORITY", "DXGI_RECLAIM_RESOURCE_RESULTS", "DXGI_RESIDENCY",
            "DXGI_RESOURCE_PRIORITY", "DXGI_SCALING", "DXGI_SWAP_EFFECT", "DXGI_USAGE",
        ],
        values: Values::Any,
        why: CHECKED_BY_SYSTEM,
    },
    // https://learn.microsoft.com/windows/win32/direct3ddxgi/dxgi-present: test, sequencing, waiting, stereo and tearing.
    Judged { enumerations: &["DXGI_PRESENT"], values: Values::Any, why: "how a frame is shown: testing, sequencing, waiting, stereo and tearing; the system refuses a combination it does not know" },
    // https://learn.microsoft.com/windows/win32/api/dxgi/ne-dxgi-dxgi_swap_chain_flag: GDI_COMPATIBLE allows GetDC, which is
    // never called; FRAME_LATENCY_WAITABLE_OBJECT hands out an event handle the kernel checks.
    Judged { enumerations: &["DXGI_SWAP_CHAIN_FLAG"], values: Values::Any, why: "a swap chain's options; GDI_COMPATIBLE allows only GetDC, which the glue never calls, and the waitable object is an event handle the kernel checks" },
    // https://learn.microsoft.com/windows/win32/api/dxgi/nf-dxgi-idxgifactory-makewindowassociation.
    Judged { enumerations: &["DXGI_MWA_FLAGS"], values: Values::Any, why: "which window messages DXGI watches (Alt+Enter, print screen, window changes): its own handling, no memory or code of ours" },
    Judged { enumerations: &["DXGI_MAP_FLAGS"], values: Values::Any, why: "how a surface is mapped through DXGI, which the glue never calls (a row)" },
    Judged { enumerations: &["DXGI_INFO_QUEUE_MESSAGE_CATEGORY", "DXGI_INFO_QUEUE_MESSAGE_SEVERITY"], values: Values::Any, why: MESSAGE_KEYS },
    // https://learn.microsoft.com/windows/win32/api/dxgi1_3/nf-dxgi1_3-createdxgifactory2: DXGI_CREATE_FACTORY_DEBUG loads the
    // debug layer, a special value refused.
    Judged { enumerations: &["DXGI_CREATE_FACTORY_FLAGS"], values: Values::Declared, why: "the one declared value loads the debug layer (refused below); an undeclared bit is no option the glue can judge" },
    Judged { enumerations: &["IDXGIOutput5::DuplicateOutput1(Flags)"], values: Values::Any, why: "reserved, zero (dxgi1_5.h); the system refuses any other value" },
    Judged { enumerations: &["DXGIGetDebugInterface1(Flags)"], values: Values::Any, why: "reserved, zero (dxgi1_3.h); the system refuses any other value" },
];

/// The judgments of Direct3D 11's enumerations, and of its numbers named as flags that the metadata links to none.
const DIRECT3D_11_JUDGED: &[Judged] = &[
    Judged {
        enumerations: &[
            "D3D11_AUTHENTICATED_CHANNEL_TYPE", "D3D11_BLEND", "D3D11_BLEND_OP", "D3D11_COMPARISON_FUNC", "D3D11_CONSERVATIVE_RASTERIZATION_MODE",
            "D3D11_CONTEXT_TYPE", "D3D11_COUNTER", "D3D11_COUNTER_TYPE", "D3D11_CRYPTO_SESSION_STATUS", "D3D11_CULL_MODE", "D3D11_DEPTH_WRITE_MASK",
            "D3D11_DEVICE_CONTEXT_TYPE", "D3D11_DSV_DIMENSION", "D3D11_FEATURE", "D3D11_FEATURE_VIDEO", "D3D11_FILL_MODE", "D3D11_FILTER",
            "D3D11_INPUT_CLASSIFICATION", "D3D11_LOGIC_OP", "D3D11_QUERY", "D3D11_RESOURCE_DIMENSION", "D3D11_RTV_DIMENSION", "D3D11_SHADER_TYPE",
            "D3D11_STENCIL_OP", "D3D11_TEXTURE_ADDRESS_MODE", "D3D11_TEXTURE_LAYOUT", "D3D11_TRACE_GS_INPUT_PRIMITIVE", "D3D11_TRACE_REGISTER_TYPE",
            "D3D11_UAV_DIMENSION", "D3D11_USAGE", "D3D11_VDOV_DIMENSION", "D3D11_VIDEO_DECODER_BUFFER_TYPE", "D3D11_VIDEO_FRAME_FORMAT",
            "D3D11_VIDEO_PROCESSOR_ALPHA_FILL_MODE", "D3D11_VIDEO_PROCESSOR_FILTER", "D3D11_VIDEO_PROCESSOR_OUTPUT_RATE", "D3D11_VIDEO_PROCESSOR_ROTATION",
            "D3D11_VIDEO_PROCESSOR_STEREO_FLIP_MODE", "D3D11_VIDEO_PROCESSOR_STEREO_FORMAT", "D3D11_VIDEO_USAGE", "D3D11_VPIV_DIMENSION", "D3D11_VPOV_DIMENSION",
            "D3D11_CPU_ACCESS_FLAG", "D3D11_CRYPTO_SESSION_KEY_EXCHANGE_FLAGS", "D3D11_RLDO_FLAGS",
        ],
        values: Values::Any,
        why: CHECKED_BY_SYSTEM,
    },
    // https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_bind_flag.
    Judged { enumerations: &["D3D11_BIND_FLAG"], values: Values::Any, why: "the pipeline stages a resource may be bound to; the system checks them against the description and the format" },
    // https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_resource_misc_flag: sharing makes handles the kernel
    // checks; GDI_COMPATIBLE allows only GetDC, which the glue never calls.
    Judged { enumerations: &["D3D11_RESOURCE_MISC_FLAG"], values: Values::Any, why: "a resource's options: sharing makes handles the kernel checks, GDI_COMPATIBLE allows only GetDC (never called), tiling binds GPU memory only" },
    Judged { enumerations: &["D3D11_FENCE_FLAG"], values: Values::Any, why: "a fence's sharing (handles the kernel checks) and monitoring; no memory of ours" },
    // https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_map: the exception line's lending depends on the value.
    Judged { enumerations: &["D3D11_MAP"], values: Values::Declared, why: "Map's exception line lends for reading only, writing, or a cleared shadow by the value: only the five declared values pass" },
    Judged { enumerations: &["D3D11_MESSAGE_CATEGORY", "D3D11_MESSAGE_ID", "D3D11_MESSAGE_SEVERITY"], values: Values::Any, why: MESSAGE_KEYS },
    // https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_create_device_flag.
    Judged { enumerations: &["D3D11_CREATE_DEVICE_FLAG"], values: Values::Declared, why: "the device's options, two of them special (below); an undeclared bit is no option the glue can judge" },
    // The numbers named as flags, masks or options that the metadata links to no enumeration (d3d11.h names each's enumeration).
    Judged { enumerations: &["ID3D11Debug::SetFeatureMask(Mask)"], values: Values::Any, why: "D3D11_DEBUG_FEATURE: flushing, finishing or presenting after each operation, and how offered resources are discarded (none is ever offered: a row)" },
    Judged {
        enumerations: &[
            "ID3D11Device::CreateDeferredContext(ContextFlags)", "ID3D11Device1::CreateDeferredContext1(ContextFlags)", "ID3D11Device2::CreateDeferredContext2(ContextFlags)",
            "ID3D11Device3::CreateDeferredContext3(ContextFlags)",
        ],
        values: Values::Any,
        why: "reserved, zero; the method is never called (a row)",
    },
    Judged { enumerations: &["ID3D11Device::SetExceptionMode(RaiseFlags)"], values: Values::Any, why: "D3D11_RAISE_FLAG: every nonzero value is refused by its row" },
    Judged { enumerations: &["ID3D11Device1::CreateDeviceContextState(Flags)"], values: Values::Any, why: "D3D11_1_CREATE_DEVICE_CONTEXT_STATE_FLAG: SINGLETHREADED is cleared by its row; the system refuses an undeclared bit" },
    Judged { enumerations: &["ID3D11Device2::CheckMultisampleQualityLevels1(Flags)"], values: Values::Any, why: "D3D11_CHECK_MULTISAMPLE_QUALITY_LEVELS_FLAG: whether to ask for tiled resources; a question only" },
    Judged { enumerations: &["ID3D11DeviceContext::Map(MapFlags)"], values: Values::Any, why: "D3D11_MAP_FLAG_DO_NOT_WAIT: whether Map waits for the GPU; the lending is the same" },
    Judged { enumerations: &["ID3D11DeviceContext::GetData(GetDataFlags)"], values: Values::Any, why: "D3D11_ASYNC_GETDATA_DONOTFLUSH: whether GetData flushes; the bytes are the glue's own buffer either way" },
    Judged { enumerations: &["ID3D11DeviceContext::OMSetBlendState(SampleMask)"], values: Values::Any, why: "a mask of the samples written, a number" },
    Judged { enumerations: &["ID3D11DeviceContext::ClearDepthStencilView(ClearFlags)"], values: Values::Any, why: "D3D11_CLEAR_FLAG: depth, stencil or both" },
    Judged { enumerations: &["ID3D11DeviceContext1::CopySubresourceRegion1(CopyFlags)", "ID3D11DeviceContext1::UpdateSubresource1(CopyFlags)"], values: Values::Any, why: "D3D11_COPY_FLAGS: DISCARD is refused by its row (it may drop memory a view holds); NO_OVERWRITE only promises the GPU" },
    Judged {
        enumerations: &[
            "ID3D11DeviceContext2::UpdateTileMappings(Flags)", "ID3D11DeviceContext2::CopyTileMappings(Flags)", "ID3D11DeviceContext2::CopyTiles(Flags)",
            "ID3D11DeviceContext2::UpdateTiles(Flags)",
        ],
        values: Values::Any,
        why: "D3D11_TILE_MAPPING_FLAG and D3D11_TILE_COPY_FLAG: the GPU's tile pool, never mapped for the CPU",
    },
    Judged {
        enumerations: &[
            "ID3D11RefDefaultTrackingOptions::SetTrackingOptions(ResourceTypeFlags)", "ID3D11RefDefaultTrackingOptions::SetTrackingOptions(Options)",
            "ID3D11RefTrackingOptions::SetTrackingOptions(uOptions)", "ID3D11TracingDevice::SetShaderTrackingOptionsByType(ResourceTypeFlags)",
            "ID3D11TracingDevice::SetShaderTrackingOptionsByType(Options)", "ID3D11TracingDevice::SetShaderTrackingOptions(Options)",
        ],
        values: Values::Any,
        why: "D3D11_SHADER_TRACKING_OPTIONS and resource types: what the reference rasterizer's tracing records, its own bookkeeping",
    },
];

/// Values the calling glue clears or refuses.
const DIRECTX_GRAPHICS_INFRASTRUCTURE_SPECIALS: &[SpecialValue] = &[
    SpecialValue { enumeration: "DXGI_CREATE_FACTORY_FLAGS", value: "DXGI_CREATE_FACTORY_DEBUG", rule: Special::Refuse, why: "the debug layer breaks (a breakpoint exception through our frames) when the DirectX control panel's settings in the registry tell it to: a setting outside the program", test: "the_debug_layer_is_refused" },
];

/// Values the calling glue clears or refuses.
const DIRECT3D_11_SPECIALS: &[SpecialValue] = &[
    // https://learn.microsoft.com/windows/win32/api/d3d11/ne-d3d11-d3d11_create_device_flag.
    SpecialValue { enumeration: "D3D11_CREATE_DEVICE_FLAG", value: "D3D11_CREATE_DEVICE_SINGLETHREADED", rule: Special::Clear, why: "makes the device and its context single threaded; a free-threaded device serves a single-threaded caller the same", test: "a_single_threaded_device_is_asked_free_threaded" },
    SpecialValue { enumeration: "D3D11_CREATE_DEVICE_FLAG", value: "D3D11_CREATE_DEVICE_DEBUG", rule: Special::Refuse, why: "the debug layer breaks (a breakpoint exception through our frames) when the DirectX control panel's settings in the registry tell it to: a setting outside the program", test: "the_debug_layer_is_refused" },
];

/// What objects of these interfaces do: describe, name and hold themselves.
const OBJECTS_ONLY: &str = "each method describes, names, compares or holds its own object, or stores private data the system copies (an interface stored is held by the system's own reference): none ends memory lent, changes how threads may call, raises an exception or runs code of ours";

/// d3dcommon's effects.
const DIRECT3D_COMMON_EFFECTS: &[Effects] = &[
    Effects { interfaces: &["ID3DBlob"], why: "a block of bytes: its address (a kind lent until Release, refused) and its size" },
    Effects { interfaces: &["ID3DDestructionNotifier"], why: "registers callbacks, whose function pointer is a kind the system trusts (refused by default), and unregisters them by a number" },
];

/// DXGI's effects.
const DIRECTX_GRAPHICS_INFRASTRUCTURE_EFFECTS: &[Effects] = &[
    Effects { interfaces: &["IDXGIObject", "IDXGIDeviceSubObject", "IDXGIResource", "IDXGIResource1", "IDXGIKeyedMutex"], why: OBJECTS_ONLY },
    Effects { interfaces: &["IDXGISurface", "IDXGISurface1", "IDXGISurface2"], why: "a texture's surface: its description and its resource; Map, Unmap, GetDC and ReleaseDC are never called (rows)" },
    Effects { interfaces: &["IDXGIAdapter", "IDXGIAdapter1", "IDXGIAdapter2", "IDXGIAdapter3", "IDXGIAdapter4"], why: "an adapter's description, outputs, memory budget and events (handles the kernel checks)" },
    Effects { interfaces: &["IDXGIOutput", "IDXGIOutput1", "IDXGIOutput2", "IDXGIOutput3", "IDXGIOutput4", "IDXGIOutput5", "IDXGIOutput6"], why: "a monitor's modes, gamma, ownership for full screen and duplication: state of the display, no memory or code of ours" },
    Effects { interfaces: &["IDXGIOutputDuplication"], why: "the desktop's frames, as objects; mapping the desktop's memory is never called (rows)" },
    Effects { interfaces: &["IDXGIDevice", "IDXGIDevice1", "IDXGIDevice2", "IDXGIDevice3", "IDXGIDevice4"], why: "a device's adapter, priorities, frame latency and residency; offering and reclaiming are never called (rows); Trim frees only the driver's own allocations" },
    Effects { interfaces: &["IDXGIFactory", "IDXGIFactory1", "IDXGIFactory2", "IDXGIFactory3", "IDXGIFactory4", "IDXGIFactory5", "IDXGIFactory6", "IDXGIFactory7", "IDXGIFactoryMedia"], why: "makes adapters and swap chains and watches windows and events (handles the kernel checks); a software adapter's module is a kind the system trusts (refused by default)" },
    Effects { interfaces: &["IDXGISwapChain", "IDXGISwapChain1", "IDXGISwapChain2", "IDXGISwapChain3", "IDXGISwapChain4", "IDXGISwapChainMedia", "IDXGIDecodeSwapChain"], why: "presents and resizes: ResizeBuffers fails while any reference to a buffer is held, and lent memory holds its resource; the waitable object is an event handle the kernel checks" },
    Effects { interfaces: &["IDXGIDisplayControl", "IDXGraphicsAnalysis"], why: "whether stereo is on, and a tool's capture: state outside our memory" },
    Effects { interfaces: &["IDXGIDebug", "IDXGIDebug1"], why: "reports live objects and tracks leaks: it changes no count and runs nothing of ours" },
    Effects { interfaces: &["IDXGIInfoQueue"], why: "stores, filters and reports messages; turning a break on is refused (rows)" },
    Effects { interfaces: &["CreateDXGIFactory", "CreateDXGIFactory1", "CreateDXGIFactory2", "DXGIDeclareAdapterRemovalSupport", "DXGIDisableVBlankVirtualization", "DXGIGetDebugInterface1"], why: "make a factory or a debug interface, or set a process-wide option of display behavior; the debug layer is refused (a special value)" },
];

/// Direct3D 11's effects.
const DIRECT3D_11_EFFECTS: &[Effects] = &[
    Effects {
        interfaces: &[
            "ID3D11DeviceChild", "ID3D11Asynchronous", "ID3D11Query", "ID3D11Query1", "ID3D11Counter", "ID3D11BlendState", "ID3D11BlendState1",
            "ID3D11DepthStencilState", "ID3D11RasterizerState", "ID3D11RasterizerState1", "ID3D11RasterizerState2", "ID3D11SamplerState", "ID3D11Resource",
            "ID3D11Buffer", "ID3D11Texture1D", "ID3D11Texture2D", "ID3D11Texture2D1", "ID3D11Texture3D", "ID3D11Texture3D1", "ID3D11View",
            "ID3D11DepthStencilView", "ID3D11RenderTargetView", "ID3D11RenderTargetView1", "ID3D11ShaderResourceView", "ID3D11ShaderResourceView1",
            "ID3D11UnorderedAccessView", "ID3D11UnorderedAccessView1", "ID3D11ClassInstance", "ID3D11ClassLinkage", "ID3D11CommandList",
            "ID3D11VideoDecoder", "ID3D11VideoDecoderOutputView", "ID3D11VideoProcessor", "ID3D11VideoProcessorEnumerator", "ID3D11VideoProcessorEnumerator1",
            "ID3D11VideoProcessorInputView", "ID3D11VideoProcessorOutputView", "ID3D11AuthenticatedChannel", "ID3D11CryptoSession", "ID3DUserDefinedAnnotation",
        ],
        why: OBJECTS_ONLY,
    },
    Effects { interfaces: &["ID3D11Fence"], why: "a fence's value, its shared handle and an event it signals (a handle the kernel checks)" },
    Effects { interfaces: &["ID3D11Device", "ID3D11Device1", "ID3D11Device2", "ID3D11Device3", "ID3D11Device4", "ID3D11Device5"], why: "makes objects, answers questions of support, opens shared objects by handles the kernel checks; deferred contexts are never made and exceptions never turned on (rows), a context state is asked free threaded (a row), and CPU access to a mapped default resource takes raw pointers (refused by default)" },
    Effects { interfaces: &["ID3D11DeviceContext", "ID3D11DeviceContext1", "ID3D11DeviceContext2", "ID3D11DeviceContext3", "ID3D11DeviceContext4"], why: "binds, draws, copies and signals; Map and Unmap are the exception line's, Discard is never called and a discarding copy refused (rows); a context state made through the glue is never single threaded; command lists come only from deferred contexts, which are never made" },
    Effects { interfaces: &["ID3D11Multithread"], why: "the device's lock: protection is answered on, Enter and Leave are never called (rows)" },
    Effects { interfaces: &["ID3D11Debug", "ID3D11RefDefaultTrackingOptions", "ID3D11RefTrackingOptions", "ID3D11TracingDevice", "ID3D11ShaderTrace", "ID3D11ShaderTraceFactory"], why: "the debug and tracing layers' reports and options: flushing after each operation and tracing records change no object, memory or thread we rely on" },
    Effects { interfaces: &["ID3D11InfoQueue"], why: "stores, filters and reports messages; turning a break on is refused (rows)" },
    Effects { interfaces: &["ID3D11SwitchToRef"], why: "switching to the reference rasterizer is never called (a row); reading the switch is a question" },
    Effects { interfaces: &["ID3D11VideoContext", "ID3D11VideoContext1", "ID3D11VideoContext2", "ID3D11VideoContext3", "ID3D11VideoDevice", "ID3D11VideoDevice1", "ID3D11VideoDevice2"], why: "decodes and processes video on objects of the API; the decoder's buffers are never lent (rows) and its extensions take raw pointers (refused by default)" },
    Effects { interfaces: &["D3D11CreateDevice", "D3D11CreateDeviceAndSwapChain"], why: "make a device (and a swap chain); a module is refused (rows), single threading cleared and the debug layer refused (special values)" },
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
    every_kind: &[],
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

/// The interfaces whose entries the hostile C caller of the whole APIs' glue set can never reach, each with why: no
/// entry or export of the set hands out an object that answers them. The generator works the set out from what each
/// glued entry hands out and fails when it differs from this list; the C caller's test excludes exactly these cases by
/// name and fails when any other case or export is never reached.
pub const UNREACHABLE_IN_WHOLE: &[(&str, &str)] = &[(
    "ID3DBlob",
    "a program gets a blob from the shader compiler (d3dcompiler_47.dll: D3DCompile, D3DCreateBlob) or Direct3D 12's serializers, other APIs left out of this set; no method or export of Direct3D 11, DXGI or d3dcommon hands one out, and no object of theirs answers it",
)];

/// The same for the spike's slice: its methods make devices, contexts, buffers, textures, views, queries and swap
/// chains only, and its faces leave out the resources' destruction notifier.
pub const UNREACHABLE_IN_SLICE: &[(&str, &str)] = &[
    ("ID3DBlob", "no method of the slice hands out a blob (nor of the whole set: see UNREACHABLE_IN_WHOLE)"),
    ("ID3DDestructionNotifier", "the slice's resources answer no destruction notifier: its faces are the spike's (Multithread on the context, DXGI's surface on a 2D texture)"),
    ("IDXGIAdapter", "the slice takes an adapter only into D3D11CreateDevice; no method of it hands one out, and its device answers no IDXGIDevice (the spike's faces)"),
];

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
