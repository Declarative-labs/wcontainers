/* A stand-in for a game: it finds D3D11CreateDevice and drives the slice through raw COM calls, as a C program does.
   The same functions run on the fake directly, on our front through the serving glue, and on the fake through both
   glues. */
#include <pthread.h>
#include <stdatomic.h>
#include <stdio.h>
#include <time.h>

#include "d3d11_slice.h"

static int failures;
static char *report_text;
static size_t report_capacity;

static void failed(int line, const char *condition) {
    if (failures++ == 0 && report_text) snprintf(report_text, report_capacity, "game.c:%d: %s", line, condition);
}

#define CHECK(condition)                                                                                             \
    do {                                                                                                             \
        if (!(condition)) failed(__LINE__, #condition);                                                              \
    } while (0)

static const GUID KEY = {0x5eed0002, 1, 2, {3, 3, 3, 3, 3, 3, 3, 3}};
static const GUID OTHER_KEY = {0x5eed0003, 1, 2, {3, 3, 3, 3, 3, 3, 3, 3}};

/* An object the game implements: Direct3D keeps it, and its release calls Direct3D again when asked to. */
typedef struct {
    Entry *table;
    atomic_uint references;
    void *call_back_on;
    int calls;
} Kept;

static HRESULT kept_query_interface(void *this, const GUID *iid, void **out) {
    if (!guid_equal(iid, &IID_IUnknown)) {
        *out = NULL;
        return E_NOINTERFACE;
    }
    atomic_fetch_add(&((Kept *)this)->references, 1);
    *out = this;
    return S_OK;
}

static uint32_t kept_add_ref(void *this) { return atomic_fetch_add(&((Kept *)this)->references, 1) + 1; }

static uint32_t kept_release(void *this) {
    Kept *kept = this;
    uint32_t count = atomic_fetch_sub(&kept->references, 1) - 1;
    void *child = kept->call_back_on;
    if (child) {
        /* A call arriving while the call that released us is still in progress. */
        kept->call_back_on = NULL;
        void *device = NULL;
        GetDevice(child, &device);
        if (device) Release(device);
        SetPrivateDataInterface(child, &OTHER_KEY, NULL);
        kept->calls++;
    }
    return count;
}

static Entry kept_table[3] = {(Entry)kept_query_interface, (Entry)kept_add_ref, (Entry)kept_release};

static HRESULT create_device(CreateDevice create, void **device, void **context) {
    int32_t levels[] = {D3D_FEATURE_LEVEL_11_0}, level = 0;
    HRESULT status = create(NULL, D3D_DRIVER_TYPE_HARDWARE, NULL, 0, levels, 1, D3D11_SDK_VERSION, device, &level, context);
    CHECK(status == S_OK && *device && *context && level == D3D_FEATURE_LEVEL_11_0);
    return status;
}

int game_play(CreateDevice create, char *report, size_t capacity) {
    failures = 0;
    report_text = report;
    report_capacity = capacity;
    void *device = NULL, *context = NULL, *same = NULL, *parent = NULL;
    if (create_device(create, &device, &context) != S_OK || !device || !context) return failures;

    /* Identity: QueryInterface, GetImmediateContext and GetDevice give the pointers already handed out. */
    CHECK(QueryInterface(device, &IID_ID3D11Device, &same) == S_OK && same == device);
    Release(same);
    CHECK(QueryInterface(device, &IID_IUnknown, &same) == S_OK && same == device);
    Release(same);
    same = device;
    CHECK(QueryInterface(device, &IID_ID3D11Buffer, &same) == E_NOINTERFACE && same == NULL);
    GetImmediateContext(device, &same);
    CHECK(same == context);
    Release(same);
    GetDevice(context, &parent);
    CHECK(parent == device);
    Release(parent);

    /* Initial bytes the game owns and changes right after the call. */
    uint8_t source[16];
    for (int at = 0; at < 16; at++) source[at] = (uint8_t)at;
    BUFFER_DESC staging = {16, D3D11_USAGE_STAGING, 0, D3D11_CPU_ACCESS_READ | D3D11_CPU_ACCESS_WRITE, 0, 0};
    SUBRESOURCE_DATA initial = {source, 0, 0};
    void *buffer = NULL;
    CHECK(CreateBuffer(device, &staging, &initial, &buffer) == S_OK && buffer);
    if (!buffer) return failures;
    memset(source, 0xff, sizeof source);
    BUFFER_DESC seen = {0};
    BufferGetDesc(buffer, &seen);
    CHECK(memcmp(&seen, &staging, sizeof seen) == 0);
    int32_t dimension = 0;
    GetType(buffer, &dimension);
    CHECK(dimension == D3D11_RESOURCE_DIMENSION_BUFFER);
    GetDevice(buffer, &parent);
    CHECK(parent == device);
    Release(parent);
    CHECK(QueryInterface(buffer, &IID_ID3D11Resource, &same) == S_OK && same == buffer);
    Release(same);

    /* Memory lent from Map to Unmap. */
    MAPPED_SUBRESOURCE mapped = {0};
    CHECK(Map(context, buffer, 0, D3D11_MAP_READ_WRITE, 0, &mapped) == S_OK && mapped.pData && mapped.RowPitch == 16);
    if (mapped.pData) {
        uint8_t *bytes = mapped.pData;
        int initial_seen = 1;
        for (int at = 0; at < 16; at++) initial_seen &= bytes[at] == at;
        CHECK(initial_seen);
        for (int at = 0; at < 16; at++) bytes[at] = (uint8_t)(100 + at);
    }
    Unmap(context, buffer, 0);
    mapped.pData = NULL;
    CHECK(Map(context, buffer, 0, D3D11_MAP_READ, 0, &mapped) == S_OK && mapped.pData);
    if (mapped.pData) {
        uint8_t *bytes = mapped.pData;
        int written_seen = 1;
        for (int at = 0; at < 16; at++) written_seen &= bytes[at] == 100 + at;
        CHECK(written_seen);
    }
    Unmap(context, buffer, 0);

    /* A map the usage does not allow. */
    BUFFER_DESC fixed_desc = {16, D3D11_USAGE_DEFAULT, D3D11_BIND_VERTEX_BUFFER, 0, 0, 0};
    void *fixed = NULL;
    CHECK(CreateBuffer(device, &fixed_desc, NULL, &fixed) == S_OK && fixed);
    CHECK(Map(context, fixed, 0, D3D11_MAP_WRITE_DISCARD, 0, &mapped) == E_INVALIDARG);

    /* An array of structures, then draws, counted by a query the context receives. */
    BUFFER_DESC dynamic_desc = {64, D3D11_USAGE_DYNAMIC, D3D11_BIND_VERTEX_BUFFER, D3D11_CPU_ACCESS_WRITE, 0, 0};
    void *dynamic = NULL;
    CHECK(CreateBuffer(device, &dynamic_desc, NULL, &dynamic) == S_OK && dynamic);
    void *bound[2] = {dynamic, fixed};
    uint32_t strides[2] = {16, 8}, offsets[2] = {0, 4};
    IASetVertexBuffers(context, 0, 2, bound, strides, offsets);
    QUERY_DESC statistics_desc = {D3D11_QUERY_PIPELINE_STATISTICS, 0};
    void *statistics = NULL;
    CHECK(CreateQuery(device, &statistics_desc, &statistics) == S_OK && statistics);
    CHECK(GetDataSize(statistics) == sizeof(PIPELINE_STATISTICS));
    Begin(context, statistics);
    Draw(context, 3, 0);
    Draw(context, 6, 3);
    End(context, statistics);
    PIPELINE_STATISTICS result = {{0}};
    HRESULT status = S_FALSE;
    for (int tries = 0; tries < 100000 && status == S_FALSE; tries++)
        status = GetData(context, statistics, &result, sizeof result, 0);
    CHECK(status == S_OK && result.counts[0] == 9);
    CHECK(GetData(context, statistics, &result, sizeof(uint64_t), 0) == E_INVALIDARG);
    CHECK(Map(context, dynamic, 0, D3D11_MAP_WRITE_DISCARD, 0, &mapped) == S_OK && mapped.pData);
    if (mapped.pData) memset(mapped.pData, 7, 64);
    Unmap(context, dynamic, 0);

    /* An object the game implements, which Direct3D keeps; its release calls Direct3D while the release's own
       call is in progress. */
    Kept kept = {kept_table, 1, NULL, 0};
    CHECK(SetPrivateDataInterface(buffer, &KEY, &kept) == S_OK);
    CHECK(atomic_load(&kept.references) == 2);
    kept.call_back_on = buffer;
    CHECK(SetPrivateDataInterface(buffer, &KEY, NULL) == S_OK);
    CHECK(atomic_load(&kept.references) == 1 && kept.calls == 1);
    CHECK(SetPrivateDataInterface(buffer, &KEY, &kept) == S_OK);
    CHECK(Release(buffer) == 0);
    CHECK(atomic_load(&kept.references) == 1);

    Release(statistics);
    Release(dynamic);
    Release(fixed);
    Release(context);
    Release(device);
    return failures;
}

/* What a hostile or careless program does: pointers that are not ours, stale ones, and arguments out of range.
   Only for our glue: the fake, like a real driver, may crash on them. */
int game_hostile(CreateDevice served, CreateDevice foreign, char *report, size_t capacity) {
    failures = 0;
    report_text = report;
    report_capacity = capacity;
    void *device = NULL, *context = NULL, *foreign_device = NULL, *foreign_context = NULL;
    if (create_device(served, &device, &context) != S_OK) return failures;
    if (create_device(foreign, &foreign_device, &foreign_context) != S_OK) return failures;
    BUFFER_DESC dynamic_desc = {64, D3D11_USAGE_DYNAMIC, D3D11_BIND_VERTEX_BUFFER, D3D11_CPU_ACCESS_WRITE, 0, 0};
    void *buffer = NULL, *foreign_buffer = NULL;
    CHECK(CreateBuffer(device, &dynamic_desc, NULL, &buffer) == S_OK && buffer);
    CHECK(CreateBuffer(foreign_device, &dynamic_desc, NULL, &foreign_buffer) == S_OK && foreign_buffer);

    /* A wrong pointer as `this`: our entries find no object. */
    uint64_t not_an_object[4] = {0};
    CountFunction add_ref = ENTRY(buffer, 1, CountFunction);
    CHECK(add_ref(not_an_object) == 0);
    ENTRY(context, 13, DrawFunction)(not_an_object, 3, 0);
    BUFFER_DESC desc = {0xdead, 0, 0, 0, 0, 0};
    ENTRY(buffer, 10, GetBufferDescFunction)(not_an_object, &desc);
    /* Found nothing: the out-parameter is written on every path, here with zeros. */
    CHECK(desc.ByteWidth == 0);

    /* A foreign object where one of ours is expected: bound as null, refused by Map. */
    void *list[1] = {foreign_buffer};
    uint32_t strides[1] = {16}, offsets[1] = {0};
    IASetVertexBuffers(context, 0, 1, list, strides, offsets);
    MAPPED_SUBRESOURCE mapped = {0};
    CHECK(Map(context, foreign_buffer, 0, D3D11_MAP_WRITE_DISCARD, 0, &mapped) == E_INVALIDARG);
    /* Slots past the last: ignored. */
    IASetVertexBuffers(context, 31, 2, list, strides, offsets);

    /* A stale pointer: the object's word outlives it, so the call finds nothing. */
    void *stale = buffer;
    CHECK(Release(buffer) == 0);
    CHECK(add_ref(stale) == 0);
    CHECK(Release(stale) == 0);
    desc.ByteWidth = 0xdead;
    BufferGetDesc(stale, &desc);
    CHECK(desc.ByteWidth == 0);
    CHECK(Map(context, stale, 0, D3D11_MAP_WRITE_DISCARD, 0, &mapped) == E_INVALIDARG);

    Release(foreign_buffer);
    Release(foreign_context);
    Release(foreign_device);
    Release(context);
    Release(device);
    return failures;
}

typedef struct {
    void *device;
    void *buffer;
    uint32_t iterations;
    atomic_int failures;
} Shared;

static void *hammer(void *argument) {
    Shared *shared = argument;
    BUFFER_DESC desc = {64, D3D11_USAGE_DYNAMIC, D3D11_BIND_VERTEX_BUFFER, D3D11_CPU_ACCESS_WRITE, 0, 0};
    for (uint32_t at = 0; at < shared->iterations; at++) {
        AddRef(shared->buffer);
        void *buffer = NULL, *device = NULL, *same = NULL;
        if (CreateBuffer(shared->device, &desc, NULL, &buffer) != S_OK || !buffer) {
            atomic_fetch_add(&shared->failures, 1);
            Release(shared->buffer);
            continue;
        }
        GetDevice(buffer, &device);
        if (device != shared->device) atomic_fetch_add(&shared->failures, 1);
        if (device) Release(device);
        if (QueryInterface(buffer, &IID_ID3D11Buffer, &same) != S_OK || same != buffer)
            atomic_fetch_add(&shared->failures, 1);
        else Release(same);
        if (Release(buffer) != 0) atomic_fetch_add(&shared->failures, 1);
        Release(shared->buffer);
    }
    return NULL;
}

/* Calls arriving from many threads at once on one device and one shared buffer, while the main thread draws. */
int game_hammer(CreateDevice create, uint32_t threads, uint32_t iterations) {
    failures = 0;
    report_text = NULL;
    void *device = NULL, *context = NULL;
    if (create_device(create, &device, &context) != S_OK) return 1;
    BUFFER_DESC desc = {64, D3D11_USAGE_DYNAMIC, D3D11_BIND_VERTEX_BUFFER, D3D11_CPU_ACCESS_WRITE, 0, 0};
    Shared shared = {device, NULL, iterations, 0};
    if (CreateBuffer(device, &desc, NULL, &shared.buffer) != S_OK) return 1;
    pthread_t workers[64];
    if (threads > 64) threads = 64;
    for (uint32_t at = 0; at < threads; at++) pthread_create(&workers[at], NULL, hammer, &shared);
    for (uint32_t at = 0; at < iterations; at++) Draw(context, 3, 0);
    for (uint32_t at = 0; at < threads; at++) pthread_join(workers[at], NULL);
    int total = atomic_load(&shared.failures) + failures;
    if (Release(shared.buffer) != 0) total++;
    Release(context);
    Release(device);
    return total;
}

static uint64_t now(void) {
    struct timespec time;
    clock_gettime(CLOCK_MONOTONIC, &time);
    return (uint64_t)time.tv_sec * 1000000000u + (uint64_t)time.tv_nsec;
}

/* Times `iterations` of one operation, in nanoseconds; an operation that pairs two calls counts as one. */
uint64_t game_time(CreateDevice create, uint32_t operation, uint64_t iterations) {
    void *device = NULL, *context = NULL;
    if (create_device(create, &device, &context) != S_OK) return 0;
    BUFFER_DESC dynamic_desc = {64, D3D11_USAGE_DYNAMIC, D3D11_BIND_VERTEX_BUFFER, D3D11_CPU_ACCESS_WRITE, 0, 0};
    BUFFER_DESC small_desc = {16, D3D11_USAGE_DEFAULT, D3D11_BIND_VERTEX_BUFFER, 0, 0, 0};
    uint8_t initial_bytes[16] = {0};
    SUBRESOURCE_DATA initial = {initial_bytes, 0, 0};
    void *dynamic = NULL, *fixed = NULL, *object = NULL;
    CreateBuffer(device, &dynamic_desc, NULL, &dynamic);
    CreateBuffer(device, &small_desc, NULL, &fixed);
    void *bound[2] = {dynamic, fixed};
    uint32_t strides[2] = {16, 8}, offsets[2] = {0, 4};
    MAPPED_SUBRESOURCE mapped;
    uint64_t start = now();
    switch (operation) {
    case 0:
        for (uint64_t at = 0; at < iterations; at++) Draw(context, 3, 0);
        break;
    case 1:
        for (uint64_t at = 0; at < iterations; at++) IASetVertexBuffers(context, 0, 2, bound, strides, offsets);
        break;
    case 2:
        for (uint64_t at = 0; at < iterations; at++) {
            AddRef(dynamic);
            Release(dynamic);
        }
        break;
    case 3:
        for (uint64_t at = 0; at < iterations; at++) {
            Map(context, dynamic, 0, D3D11_MAP_WRITE_DISCARD, 0, &mapped);
            Unmap(context, dynamic, 0);
        }
        break;
    case 4:
        for (uint64_t at = 0; at < iterations; at++) {
            GetDevice(dynamic, &object);
            Release(object);
        }
        break;
    case 5:
        for (uint64_t at = 0; at < iterations; at++) {
            QueryInterface(dynamic, &IID_ID3D11Buffer, &object);
            Release(object);
        }
        break;
    case 6:
        for (uint64_t at = 0; at < iterations; at++) {
            CreateBuffer(device, &small_desc, &initial, &object);
            Release(object);
        }
        break;
    }
    uint64_t elapsed = now() - start;
    Release(fixed);
    Release(dynamic);
    Release(context);
    Release(device);
    return elapsed;
}

/* Textures through raw COM: a back buffer by GetBuffer for two interfaces of two headers, a texture's rows mapped at
   the pitch the implementation gives, and render targets and unordered access views, kept and replaced. */
int game_textures(CreateDeviceAndSwapChain create, char *report, size_t capacity) {
    failures = 0;
    report_text = report;
    report_capacity = capacity;
    SWAP_CHAIN_DESC desc = {{64, 32, 60, 1, DXGI_FORMAT_R8G8B8A8_UNORM, 0, 0}, {1, 0},
                            DXGI_USAGE_RENDER_TARGET_OUTPUT, 2, NULL, 1, 0, 0};
    int32_t levels[] = {D3D_FEATURE_LEVEL_11_0}, level = 0;
    void *swap_chain = NULL, *device = NULL, *context = NULL;
    HRESULT status = create(NULL, D3D_DRIVER_TYPE_HARDWARE, NULL, 0, levels, 1, D3D11_SDK_VERSION, &desc, &swap_chain,
                            &device, &level, &context);
    CHECK(status == S_OK && swap_chain && device && context);
    if (!swap_chain || !device || !context) return failures;

    /* GetBuffer by the interface asked for: Direct3D 11's texture, then DXGI's surface of the same object. */
    void *texture = NULL, *surface = NULL, *again = NULL, *unknown = NULL, *other = NULL, *none = (void *)1;
    CHECK(GetBuffer(swap_chain, 0, &IID_ID3D11Texture2D, &texture) == S_OK && texture);
    if (!texture) return failures;
    TEXTURE2D_DESC seen = {0};
    Texture2DGetDesc(texture, &seen);
    CHECK(seen.Width == 64 && seen.Height == 32 && seen.Format == DXGI_FORMAT_R8G8B8A8_UNORM);
    CHECK(QueryInterface(texture, &IID_IDXGISurface, &surface) == S_OK && surface && surface != texture);
    SURFACE_DESC surface_desc = {0};
    CHECK(surface && SurfaceGetDesc(surface, &surface_desc) == S_OK && surface_desc.Width == 64);
    CHECK(GetBuffer(swap_chain, 0, &IID_IDXGISurface, &again) == S_OK && again == surface);
    CHECK(surface && QueryInterface(surface, &IID_IUnknown, &unknown) == S_OK && unknown == texture);
    CHECK(GetBuffer(swap_chain, 0, &IID_ID3D11Buffer, &none) == E_NOINTERFACE && none == NULL);
    CHECK(GetBuffer(swap_chain, 7, &IID_ID3D11Texture2D, &other) < 0 && other == NULL);
    CHECK(Present(swap_chain, 0, 0) == S_OK);
    if (unknown) Release(unknown);
    if (again) Release(again);
    if (surface) Release(surface);

    /* A staging texture's rows at the pitch Map gives, the last one row long. */
    uint8_t initial_bytes[3 * 20];
    for (int at = 0; at < 60; at++) initial_bytes[at] = (uint8_t)at;
    SUBRESOURCE_DATA initial = {initial_bytes, 20, 0};
    TEXTURE2D_DESC staging = {5, 3, 1, 1, DXGI_FORMAT_R8G8B8A8_UNORM, {1, 0}, D3D11_USAGE_STAGING, 0,
                              D3D11_CPU_ACCESS_READ | D3D11_CPU_ACCESS_WRITE, 0};
    void *rows = NULL;
    CHECK(CreateTexture2D(device, &staging, &initial, &rows) == S_OK && rows);
    MAPPED_SUBRESOURCE mapped = {0};
    CHECK(rows && Map(context, rows, 0, D3D11_MAP_READ_WRITE, 0, &mapped) == S_OK && mapped.pData);
    if (rows && mapped.pData) {
        CHECK(mapped.RowPitch >= 20);
        uint8_t *bytes = mapped.pData;
        CHECK(memcmp(bytes + mapped.RowPitch, initial_bytes + 20, 20) == 0);
        for (int row = 0; row < 3; row++) memset(bytes + row * mapped.RowPitch, 0x40 + row, 20);
        Unmap(context, rows, 0);
        mapped.pData = NULL;
        CHECK(Map(context, rows, 0, D3D11_MAP_READ, 0, &mapped) == S_OK && mapped.pData);
        if (mapped.pData) CHECK(((uint8_t *)mapped.pData)[2 * mapped.RowPitch + 19] == 0x42);
        Unmap(context, rows, 0);
    }
    /* A compressed texture's second mip: 5 by 5 texels, 2 by 2 blocks of 8 bytes. */
    TEXTURE2D_DESC compressed = staging;
    compressed.Width = compressed.Height = 10;
    compressed.MipLevels = 2;
    compressed.Format = DXGI_FORMAT_BC1_UNORM;
    void *blocks = NULL;
    CHECK(CreateTexture2D(device, &compressed, NULL, &blocks) == S_OK && blocks);
    mapped.pData = NULL;
    CHECK(blocks && Map(context, blocks, 1, D3D11_MAP_WRITE, 0, &mapped) == S_OK && mapped.pData);
    if (blocks && mapped.pData) {
        for (int row = 0; row < 2; row++) memset((uint8_t *)mapped.pData + row * mapped.RowPitch, 0x77, 16);
        Unmap(context, blocks, 1);
    }

    /* Render targets with a depth stencil view, and unordered access views: replaced, then each group kept. */
    TEXTURE2D_DESC depth_desc = {64, 32, 1, 1, DXGI_FORMAT_D24_UNORM_S8_UINT, {1, 0}, D3D11_USAGE_DEFAULT,
                                 D3D11_BIND_DEPTH_STENCIL, 0, 0};
    TEXTURE2D_DESC target_desc = {64, 32, 1, 1, DXGI_FORMAT_R8G8B8A8_UNORM, {1, 0}, D3D11_USAGE_DEFAULT,
                                  D3D11_BIND_RENDER_TARGET | D3D11_BIND_UNORDERED_ACCESS, 0, 0};
    void *depth = NULL, *target = NULL, *back_view = NULL, *target_view = NULL, *depth_view = NULL, *uav = NULL,
         *uav2 = NULL;
    CHECK(CreateTexture2D(device, &depth_desc, NULL, &depth) == S_OK && depth);
    CHECK(CreateTexture2D(device, &target_desc, NULL, &target) == S_OK && target);
    CHECK(CreateRenderTargetView(device, texture, NULL, &back_view) == S_OK && back_view);
    CHECK(CreateRenderTargetView(device, target, NULL, &target_view) == S_OK && target_view);
    CHECK(CreateDepthStencilView(device, depth, NULL, &depth_view) == S_OK && depth_view);
    VIEW_DESC uav_desc = {DXGI_FORMAT_R8G8B8A8_UNORM, D3D11_UAV_DIMENSION_TEXTURE2D, {0, 0, 0}};
    CHECK(CreateUnorderedAccessView(device, target, &uav_desc, &uav) == S_OK && uav);
    CHECK(CreateUnorderedAccessView(device, target, &uav_desc, &uav2) == S_OK && uav2);
    void *targets[2] = {back_view, target_view}, *views[1] = {uav}, *views2[1] = {uav2};
    uint32_t counts[1] = {0xffffffffu};
    void *const *garbage = (void *const *)(uintptr_t)0x10;
    OMSetRenderTargetsAndUnorderedAccessViews(context, 2, targets, depth_view, 2, 1, views, counts);
    Draw(context, 3, 0);
    /* Keep the render targets: their arrays are not read, so a garbage pointer is harmless. */
    OMSetRenderTargetsAndUnorderedAccessViews(context, D3D11_KEEP_RENDER_TARGETS_AND_DEPTH_STENCIL, garbage,
                                              (void *)garbage, 3, 1, views2, counts);
    Draw(context, 3, 0);
    /* Keep the unordered access views, and bind no render target. */
    OMSetRenderTargetsAndUnorderedAccessViews(context, 0, NULL, NULL, 0, D3D11_KEEP_UNORDERED_ACCESS_VIEWS, garbage,
                                              (const uint32_t *)garbage);
    Draw(context, 3, 0);

    void *objects[] = {uav2, uav, depth_view, target_view, back_view, target, depth, blocks, rows, texture};
    for (size_t at = 0; at < sizeof objects / sizeof *objects; at++)
        if (objects[at]) Release(objects[at]);
    Release(swap_chain);
    Release(context);
    Release(device);
    return failures;
}
