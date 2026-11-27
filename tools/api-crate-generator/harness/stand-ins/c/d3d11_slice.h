/* The spike's slice of d3d11.h, as C sees it: types, constants, interface IDs, and each method called through its
   table index, as a C program calls a COM object. Declarations follow Microsoft's documented layout. */
#ifndef D3D11_SLICE_H
#define D3D11_SLICE_H

#include <stdint.h>
#include <string.h>

typedef int32_t HRESULT;
typedef void *Entry;

typedef struct {
    uint32_t data1;
    uint16_t data2, data3;
    uint8_t data4[8];
} GUID;

typedef struct {
    uint32_t ByteWidth;
    int32_t Usage;
    uint32_t BindFlags, CPUAccessFlags, MiscFlags, StructureByteStride;
} BUFFER_DESC;

typedef struct {
    const void *pSysMem;
    uint32_t SysMemPitch, SysMemSlicePitch;
} SUBRESOURCE_DATA;

typedef struct {
    void *pData;
    uint32_t RowPitch, DepthPitch;
} MAPPED_SUBRESOURCE;

typedef struct {
    int32_t Query;
    uint32_t MiscFlags;
} QUERY_DESC;

typedef struct {
    uint64_t counts[11];
} PIPELINE_STATISTICS;

typedef HRESULT (*CreateDevice)(void *, int32_t, void *, uint32_t, const int32_t *, uint32_t, uint32_t, void **,
                                int32_t *, void **);

typedef struct {
    uint32_t Count, Quality;
} SAMPLE_DESC;

typedef struct {
    uint32_t Width, Height, MipLevels, ArraySize, Format;
    SAMPLE_DESC SampleDesc;
    int32_t Usage;
    uint32_t BindFlags, CPUAccessFlags, MiscFlags;
} TEXTURE2D_DESC;

/* A view's description: its union as its largest member's words. */
typedef struct {
    uint32_t Format;
    int32_t ViewDimension;
    uint32_t Union[3];
} VIEW_DESC;

typedef struct {
    uint32_t Width, Height, Format;
    SAMPLE_DESC SampleDesc;
} SURFACE_DESC;

typedef struct {
    uint32_t Width, Height, Numerator, Denominator, Format;
    int32_t ScanlineOrdering, Scaling;
} MODE_DESC;

typedef struct {
    MODE_DESC BufferDesc;
    SAMPLE_DESC SampleDesc;
    uint32_t BufferUsage, BufferCount;
    void *OutputWindow;
    int32_t Windowed, SwapEffect;
    uint32_t Flags;
} SWAP_CHAIN_DESC;

typedef HRESULT (*CreateDeviceAndSwapChain)(void *, int32_t, void *, uint32_t, const int32_t *, uint32_t, uint32_t,
                                            const SWAP_CHAIN_DESC *, void **, void **, int32_t *, void **);

#define S_OK 0
#define S_FALSE 1
#define E_NOINTERFACE ((HRESULT)0x80004002u)
#define E_POINTER ((HRESULT)0x80004003u)
#define E_OUTOFMEMORY ((HRESULT)0x8007000Eu)
#define E_INVALIDARG ((HRESULT)0x80070057u)
#define E_FAIL ((HRESULT)0x80004005u)
#define DXGI_ERROR_INVALID_CALL ((HRESULT)0x887A0001u)

#define D3D11_SDK_VERSION 7
#define D3D_DRIVER_TYPE_HARDWARE 1
#define D3D_FEATURE_LEVEL_9_1 0x9100
#define D3D_FEATURE_LEVEL_11_0 0xb000
#define D3D_FEATURE_LEVEL_11_1 0xb100
#define D3D11_USAGE_DEFAULT 0
#define D3D11_USAGE_IMMUTABLE 1
#define D3D11_USAGE_DYNAMIC 2
#define D3D11_USAGE_STAGING 3
#define D3D11_BIND_VERTEX_BUFFER 0x1
#define D3D11_CPU_ACCESS_WRITE 0x10000
#define D3D11_CPU_ACCESS_READ 0x20000
#define D3D11_MAP_READ 1
#define D3D11_MAP_WRITE 2
#define D3D11_MAP_READ_WRITE 3
#define D3D11_MAP_WRITE_DISCARD 4
#define D3D11_MAP_WRITE_NO_OVERWRITE 5
#define D3D11_QUERY_EVENT 0
#define D3D11_QUERY_PIPELINE_STATISTICS 4
#define D3D11_RESOURCE_DIMENSION_BUFFER 1
#define D3D11_RESOURCE_DIMENSION_TEXTURE2D 3
#define D3D11_BIND_SHADER_RESOURCE 0x8
#define D3D11_BIND_RENDER_TARGET 0x20
#define D3D11_BIND_DEPTH_STENCIL 0x40
#define D3D11_BIND_UNORDERED_ACCESS 0x80
#define D3D11_UAV_DIMENSION_TEXTURE2D 4
#define D3D11_KEEP_RENDER_TARGETS_AND_DEPTH_STENCIL 0xffffffffu
#define D3D11_KEEP_UNORDERED_ACCESS_VIEWS 0xffffffffu
#define D3D11_CREATE_DEVICE_SINGLETHREADED 0x1
#define DXGI_FORMAT_R32G32B32A32_FLOAT 2
#define DXGI_FORMAT_R8G8B8A8_UNORM 28
#define DXGI_FORMAT_D24_UNORM_S8_UINT 45
#define DXGI_FORMAT_BC1_UNORM 71
#define DXGI_FORMAT_B8G8R8A8_UNORM 87
#define DXGI_USAGE_RENDER_TARGET_OUTPUT 0x20

static const GUID IID_IUnknown = {0, 0, 0, {0xc0, 0, 0, 0, 0, 0, 0, 0x46}};
static const GUID IID_ID3D11DeviceChild = {0x1841e5c8, 0x16b0, 0x489b, {0xbc, 0xc8, 0x44, 0xcf, 0xb0, 0xd5, 0xde, 0xae}};
static const GUID IID_ID3D11Device = {0xdb6f6ddb, 0xac77, 0x4e88, {0x82, 0x53, 0x81, 0x9d, 0xf9, 0xbb, 0xf1, 0x40}};
static const GUID IID_ID3D11DeviceContext = {0xc0bfa96c, 0xe089, 0x44fb, {0x8e, 0xaf, 0x26, 0xf8, 0x79, 0x61, 0x90, 0xda}};
static const GUID IID_ID3D11Resource = {0xdc8e63f3, 0xd12b, 0x4952, {0xb4, 0x7b, 0x5e, 0x45, 0x02, 0x6a, 0x86, 0x2d}};
static const GUID IID_ID3D11Buffer = {0x48570b85, 0xd1ee, 0x4fcd, {0xa2, 0x50, 0xeb, 0x35, 0x07, 0x22, 0xb0, 0x37}};
static const GUID IID_ID3D11Asynchronous = {0x4b35d0cd, 0x1e15, 0x4258, {0x9c, 0x98, 0x1b, 0x13, 0x33, 0xf6, 0xdd, 0x3b}};
static const GUID IID_ID3D11Query = {0xd6c00747, 0x87b7, 0x425e, {0xb8, 0x4d, 0x44, 0xd1, 0x08, 0x56, 0x0a, 0xfd}};
static const GUID IID_ID3D11Texture2D = {0x6f15aaf2, 0xd208, 0x4e89, {0x9a, 0xb4, 0x48, 0x95, 0x35, 0xd3, 0x4f, 0x9c}};
static const GUID IID_ID3D11View = {0x839d1216, 0xbb2e, 0x412b, {0xb7, 0xf4, 0xa9, 0xdb, 0xeb, 0xe0, 0x8e, 0xd1}};
static const GUID IID_ID3D11RenderTargetView = {0xdfdba067, 0x0b8d, 0x4865, {0x87, 0x5b, 0xd7, 0xb4, 0x51, 0x6c, 0xc1, 0x64}};
static const GUID IID_ID3D11DepthStencilView = {0x9fdac92a, 0x1876, 0x48c3, {0xaf, 0xad, 0x25, 0xb9, 0x4f, 0x84, 0xa9, 0xb6}};
static const GUID IID_ID3D11UnorderedAccessView = {0x28acf509, 0x7f5c, 0x48f6, {0x86, 0x11, 0xf3, 0x16, 0x01, 0x0a, 0x63, 0x80}};
static const GUID IID_ID3D11Multithread = {0x9b7e4e00, 0x342c, 0x4106, {0xa1, 0x9f, 0x4f, 0x27, 0x04, 0xf6, 0x89, 0xf0}};
static const GUID IID_IDXGIObject = {0xaec22fb8, 0x76f3, 0x4639, {0x9b, 0xe0, 0x28, 0xeb, 0x43, 0xa6, 0x7a, 0x2e}};
static const GUID IID_IDXGIDeviceSubObject = {0x3d3e0379, 0xf9de, 0x4d58, {0xbb, 0x6c, 0x18, 0xd6, 0x29, 0x92, 0xf1, 0xa6}};
static const GUID IID_IDXGISurface = {0xcafcb56c, 0x6ac3, 0x4889, {0xbf, 0x47, 0x9e, 0x23, 0xbb, 0xd2, 0x60, 0xec}};
static const GUID IID_IDXGISwapChain = {0x310d36a0, 0xd2e7, 0x4c0a, {0xaa, 0x04, 0x6a, 0x9d, 0x23, 0xb8, 0x88, 0x6a}};

static inline int guid_equal(const GUID *a, const GUID *b) { return memcmp(a, b, sizeof *a) == 0; }

typedef HRESULT (*QueryInterfaceFunction)(void *, const GUID *, void **);
typedef uint32_t (*CountFunction)(void *);
typedef HRESULT (*CreateBufferFunction)(void *, const BUFFER_DESC *, const SUBRESOURCE_DATA *, void **);
typedef HRESULT (*CreateQueryFunction)(void *, const QUERY_DESC *, void **);
typedef void (*GetObjectFunction)(void *, void **);
typedef HRESULT (*SetPrivateDataInterfaceFunction)(void *, const GUID *, void *);
typedef void (*GetTypeFunction)(void *, int32_t *);
typedef void (*GetBufferDescFunction)(void *, BUFFER_DESC *);
typedef uint32_t (*GetDataSizeFunction)(void *);
typedef void (*DrawFunction)(void *, uint32_t, uint32_t);
typedef HRESULT (*MapFunction)(void *, void *, uint32_t, int32_t, uint32_t, MAPPED_SUBRESOURCE *);
typedef void (*UnmapFunction)(void *, void *, uint32_t);
typedef void (*SetVertexBuffersFunction)(void *, uint32_t, uint32_t, void *const *, const uint32_t *, const uint32_t *);
typedef void (*AsynchronousFunction)(void *, void *);
typedef HRESULT (*GetDataFunction)(void *, void *, void *, uint32_t, uint32_t);
typedef HRESULT (*CreateTexture2DFunction)(void *, const TEXTURE2D_DESC *, const SUBRESOURCE_DATA *, void **);
typedef HRESULT (*CreateViewFunction)(void *, void *, const void *, void **);
typedef void (*GetTexture2DDescFunction)(void *, TEXTURE2D_DESC *);
typedef void (*OutputMergerFunction)(void *, uint32_t, void *const *, void *, uint32_t, uint32_t, void *const *,
                                     const uint32_t *);
typedef HRESULT (*SurfaceGetDescFunction)(void *, SURFACE_DESC *);
typedef HRESULT (*PresentFunction)(void *, uint32_t, uint32_t);
typedef HRESULT (*GetBufferFunction)(void *, uint32_t, const GUID *, void **);
typedef int32_t (*SetProtectedFunction)(void *, int32_t);

/* The entry at `index` of the object's table, as type `type`. */
#define ENTRY(object, index, type) ((type)((*(Entry **)(object))[index]))

#define QueryInterface(o, iid, out) ENTRY(o, 0, QueryInterfaceFunction)(o, iid, out)
#define AddRef(o) ENTRY(o, 1, CountFunction)(o)
#define Release(o) ENTRY(o, 2, CountFunction)(o)
#define CreateBuffer(d, desc, initial, out) ENTRY(d, 3, CreateBufferFunction)(d, desc, initial, out)
#define CreateQuery(d, desc, out) ENTRY(d, 24, CreateQueryFunction)(d, desc, out)
#define GetImmediateContext(d, out) ENTRY(d, 40, GetObjectFunction)(d, out)
#define GetDevice(c, out) ENTRY(c, 3, GetObjectFunction)(c, out)
#define SetPrivateDataInterface(c, key, value) ENTRY(c, 6, SetPrivateDataInterfaceFunction)(c, key, value)
#define GetType(r, out) ENTRY(r, 7, GetTypeFunction)(r, out)
#define BufferGetDesc(b, out) ENTRY(b, 10, GetBufferDescFunction)(b, out)
#define GetDataSize(q) ENTRY(q, 7, GetDataSizeFunction)(q)
#define Draw(c, count, start) ENTRY(c, 13, DrawFunction)(c, count, start)
#define Map(c, r, s, type, flags, out) ENTRY(c, 14, MapFunction)(c, r, s, type, flags, out)
#define Unmap(c, r, s) ENTRY(c, 15, UnmapFunction)(c, r, s)
#define IASetVertexBuffers(c, start, n, b, s, o) ENTRY(c, 18, SetVertexBuffersFunction)(c, start, n, b, s, o)
#define Begin(c, q) ENTRY(c, 27, AsynchronousFunction)(c, q)
#define End(c, q) ENTRY(c, 28, AsynchronousFunction)(c, q)
#define GetData(c, q, d, n, f) ENTRY(c, 29, GetDataFunction)(c, q, d, n, f)
#define CreateTexture2D(d, desc, initial, out) ENTRY(d, 5, CreateTexture2DFunction)(d, desc, initial, out)
#define CreateUnorderedAccessView(d, r, desc, out) ENTRY(d, 8, CreateViewFunction)(d, r, desc, out)
#define CreateRenderTargetView(d, r, desc, out) ENTRY(d, 9, CreateViewFunction)(d, r, desc, out)
#define CreateDepthStencilView(d, r, desc, out) ENTRY(d, 10, CreateViewFunction)(d, r, desc, out)
#define Texture2DGetDesc(t, out) ENTRY(t, 10, GetTexture2DDescFunction)(t, out)
#define OMSetRenderTargetsAndUnorderedAccessViews(c, n, r, d, s, m, u, i)                                            \
    ENTRY(c, 34, OutputMergerFunction)(c, n, r, d, s, m, u, i)
#define SurfaceGetDesc(s, out) ENTRY(s, 8, SurfaceGetDescFunction)(s, out)
#define Present(s, interval, flags) ENTRY(s, 8, PresentFunction)(s, interval, flags)
#define GetBuffer(s, index, iid, out) ENTRY(s, 9, GetBufferFunction)(s, index, iid, out)
#define SetMultithreadProtected(m, on) ENTRY(m, 5, SetProtectedFunction)(m, on)

/* What the fake saw, so that a test can tell what reached the system. */
typedef struct {
    uint32_t create_calls, create_flags, ia_calls, ia_start, ia_count, om_calls, om_targets, om_depth, om_views,
        om_first_view, mapped_now;
    int32_t create_driver_type;
    uint32_t protected_contexts;
    /* Times a context's protection was turned off, and devices asked for single threaded (D3D11_CREATE_DEVICE_SINGLETHREADED):
       both undo the free threading the calling glue's Send and Sync rest on, so a test expects 0 through the glue. */
    uint32_t unprotections, single_threaded_creates;
} Observed;

#endif
