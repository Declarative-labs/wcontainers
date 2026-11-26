/* The fuzzing drivers, through raw COM calls only, from several threads.

   fuzz_correct: a correct game that does everything the slice allows, keeping every rule the documentation states,
   and checks every answer the glue promises whatever the implementation does: a failed create leaves null, GetDesc
   is what was asked, GetType is the object's kind, GetDevice is the device that made it, IUnknown's pointer and the
   immediate context are the same each time, GetDataSize is the header's, a back buffer is the same while held, and
   a mapping holds every byte the game is told it may touch (which it then writes). Initial bytes live in a page
   that is read-only, so a write into them faults.

   fuzz_hostile: calls random entries of random objects with random arguments: `this` and every object argument
   drawn from live, released, wrong-kind, misaligned, unmapped and null pointers; counts at and past their ranges and
   the keep values; enums and flags at random; data pointers valid for what they promise, or null. It keeps only the
   rules no glue can check: a data pointer holds what its SAL annotation promises, an IUnknown* is a COM object (ours,
   even released, or its own), and mapped memory is written only between its Map and its Unmap. */
#define _GNU_SOURCE
#include <pthread.h>
#include <stdatomic.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <unistd.h>

#include "d3d11_slice.h"

atomic_ullong fuzz_calls;
atomic_uint fuzz_violations;
static char first_violation[256];
static atomic_int violation_written;

/* What the calls achieved: devices made, resources made, mappings written, views made, query results read, back
   buffers got, and the kinds of table the hostile game reached (a bit each). */
atomic_ullong fuzz_counts[7];
enum { MADE_DEVICES, MADE_RESOURCES, WROTE_MAPPINGS, MADE_VIEWS, READ_RESULTS, GOT_BACK_BUFFERS, REACHED_TABLES };
#define COUNT(what) atomic_fetch_add_explicit(&fuzz_counts[what], 1, memory_order_relaxed)
void fuzz_count(unsigned long long *out) {
    for (int at = 0; at < 7; at++) out[at] = atomic_load(&fuzz_counts[at]);
}

unsigned long long fuzz_progress(void) { return atomic_load(&fuzz_calls); }
unsigned fuzz_violation_count(void) { return atomic_load(&fuzz_violations); }
const char *fuzz_first_violation(void) { return first_violation; }

static void violated(int line, const char *what) {
    atomic_fetch_add(&fuzz_violations, 1);
    if (atomic_exchange(&violation_written, 1) == 0) snprintf(first_violation, sizeof first_violation, "fuzz.c:%d: %s", line, what);
}

#define EXPECT(condition)                                                                                            \
    do {                                                                                                             \
        if (!(condition)) violated(__LINE__, #condition);                                                            \
    } while (0)

/* One call made through the API, counted for the whole run and for its thread. */
static _Thread_local uint64_t thread_calls;
#define CALL(expression) (thread_calls++, atomic_fetch_add_explicit(&fuzz_calls, 1, memory_order_relaxed), (expression))

typedef struct {
    uint64_t state;
} Random;

static uint64_t next(Random *random) {
    uint64_t x = random->state;
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    random->state = x;
    return x * 0x2545f4914f6cdd1dull;
}

static uint32_t below(Random *random, uint32_t n) { return n ? (uint32_t)(next(random) % n) : 0; }

/* ---- Shared: read-only initial bytes, interface ids, the formats a texture is made of. */

static const uint8_t *read_only;
static const size_t READ_ONLY_BYTES = 1 << 20;

static void make_read_only(void) {
    if (read_only) return;
    uint8_t *page = mmap(NULL, READ_ONLY_BYTES, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    for (size_t at = 0; at < READ_ONLY_BYTES; at++) page[at] = (uint8_t)(at * 7);
    mprotect(page, READ_ONLY_BYTES, PROT_READ);
    read_only = page;
}

static const GUID *const IIDS[] = {&IID_IUnknown,         &IID_ID3D11DeviceChild,  &IID_ID3D11Device,
                                   &IID_ID3D11DeviceContext, &IID_ID3D11Resource,   &IID_ID3D11Buffer,
                                   &IID_ID3D11Texture2D,  &IID_ID3D11View,         &IID_ID3D11RenderTargetView,
                                   &IID_ID3D11DepthStencilView, &IID_ID3D11UnorderedAccessView,
                                   &IID_ID3D11Asynchronous, &IID_ID3D11Query,      &IID_IDXGIObject,
                                   &IID_IDXGIDeviceSubObject, &IID_IDXGISurface,   &IID_IDXGISwapChain};
enum { IID_COUNT = sizeof IIDS / sizeof *IIDS };

/* Formats with their bytes per block and block side: R32G32B32A32, R8G8B8A8, B8G8R8A8, BC1. */
static const uint32_t FORMATS[4][3] = {{2, 16, 1}, {28, 4, 1}, {87, 4, 1}, {71, 8, 4}};

static uint32_t full_chain(uint32_t width, uint32_t height) {
    uint32_t side = width > height ? width : height, levels = 0;
    while (side) {
        levels++;
        side >>= 1;
    }
    return levels;
}

/* The rows of blocks and bytes per row of a subresource, as the header's format table gives them. */
static void subresource_rows(const TEXTURE2D_DESC *desc, uint32_t subresource, uint32_t block_bytes, uint32_t side,
                             uint32_t *rows, uint32_t *row_bytes) {
    uint32_t mip = subresource % desc->MipLevels, width = desc->Width >> mip, height = desc->Height >> mip;
    width = width ? width : 1;
    height = height ? height : 1;
    *rows = (height + side - 1) / side;
    *row_bytes = (width + side - 1) / side * block_bytes;
}

/* An object the game implements and hands to Direct3D, whose release calls back into the child that held it. */
typedef struct {
    Entry *table;
    atomic_uint references;
    void *_Atomic call_back_on;
} Kept;

static HRESULT kept_query_interface(void *this, const GUID *iid, void **out) {
    if (!out) return E_POINTER;
    if (!iid || !guid_equal(iid, &IID_IUnknown)) {
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
    void *child = atomic_exchange(&kept->call_back_on, NULL);
    if (child) {
        /* A call arriving while the call that released us is in progress; the child is alive while it holds us. */
        void *device = NULL;
        GetDevice(child, &device);
        if (device) Release(device);
    }
    return count;
}

static Entry kept_table[3] = {(Entry)kept_query_interface, (Entry)kept_add_ref, (Entry)kept_release};

/* Kept objects live for the whole run: Direct3D may release them on any thread at any time. */
static Kept kept_objects[64];

static void make_kept(void) {
    for (int at = 0; at < 64; at++) {
        kept_objects[at].table = kept_table;
        atomic_store(&kept_objects[at].references, 1u << 30);
    }
}

/* ---- The correct game. */

enum { OWN = 12 };

typedef struct {
    void *object;
    int kind; /* 0 buffer, 1 texture */
    BUFFER_DESC buffer;
    TEXTURE2D_DESC texture;
    uint32_t block_bytes, side, subresources;
    void *identity;
} Resource;

typedef struct {
    void *object;
    QUERY_DESC desc;
} OwnQuery;

typedef struct {
    Random random;
    CreateDevice create;
    CreateDeviceAndSwapChain create_swap;
    uint64_t budget;
    void *device, *context, *device_identity;
    void *swap_chain;
    uint32_t back_buffers;
    Resource resources[OWN];
    void *views[OWN];
    OwnQuery queries[OWN];
} Game;

static void release_slot(void **object) {
    if (*object) CALL(Release(*object));
    *object = NULL;
}

static int make_device(Game *game) {
    int32_t levels[3] = {D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL_11_1, D3D_FEATURE_LEVEL_9_1};
    uint32_t count = below(&game->random, 4);
    int32_t level = 0x7777;
    void *device = (void *)1, *context = (void *)1, *swap_chain = (void *)1;
    HRESULT status;
    if (below(&game->random, 2)) {
        SWAP_CHAIN_DESC desc = {{1 + below(&game->random, 64), 1 + below(&game->random, 64), 60, 1, 28, 0, 0},
                                {1, 0}, DXGI_USAGE_RENDER_TARGET_OUTPUT, 1 + below(&game->random, 3), NULL, 1, 0, 0};
        status = CALL(game->create_swap(NULL, D3D_DRIVER_TYPE_HARDWARE, NULL, 0, count ? levels : NULL, count,
                                        D3D11_SDK_VERSION, &desc, &swap_chain, &device, &level, &context));
        if (status >= 0) game->back_buffers = desc.BufferCount;
        EXPECT(status >= 0 ? (swap_chain != NULL && device != NULL && context != NULL)
                           : (swap_chain == NULL && device == NULL && context == NULL));
    } else {
        swap_chain = NULL;
        status = CALL(game->create(NULL, D3D_DRIVER_TYPE_HARDWARE, NULL, 0, count ? levels : NULL, count,
                                   D3D11_SDK_VERSION, &device, &level, &context));
        EXPECT(status >= 0 ? (device != NULL && context != NULL) : (device == NULL && context == NULL));
    }
    if (status < 0) return 0;
    /* The level is one offered, or one of the default list. */
    int offered = 0;
    for (uint32_t at = 0; at < count; at++) offered |= levels[at] == level;
    EXPECT(count ? offered : (level >= D3D_FEATURE_LEVEL_9_1 && level <= D3D_FEATURE_LEVEL_11_0));
    COUNT(MADE_DEVICES);
    game->device = device;
    game->context = context;
    game->swap_chain = swap_chain;
    void *identity = NULL;
    EXPECT(CALL(QueryInterface(device, &IID_IUnknown, &identity)) == S_OK && identity == device);
    if (identity) CALL(Release(identity));
    game->device_identity = device;
    return 1;
}

static void create_buffer(Game *game, Resource *slot) {
    Random *random = &game->random;
    static const int32_t usages[4] = {D3D11_USAGE_DEFAULT, D3D11_USAGE_IMMUTABLE, D3D11_USAGE_DYNAMIC, D3D11_USAGE_STAGING};
    BUFFER_DESC desc = {1 + below(random, 4096), usages[below(random, 4)], D3D11_BIND_VERTEX_BUFFER, 0, 0, 0};
    if (desc.Usage == D3D11_USAGE_DYNAMIC) desc.CPUAccessFlags = D3D11_CPU_ACCESS_WRITE;
    if (desc.Usage == D3D11_USAGE_STAGING) {
        desc.BindFlags = 0;
        desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ | D3D11_CPU_ACCESS_WRITE;
    }
    SUBRESOURCE_DATA initial = {read_only + below(random, 4096), 0, 0};
    int with_initial = desc.Usage == D3D11_USAGE_IMMUTABLE || below(random, 2);
    void *buffer = (void *)1;
    int validate = below(random, 16) == 0;
    HRESULT status = CALL(CreateBuffer(game->device, &desc, with_initial ? &initial : NULL, validate ? NULL : &buffer));
    if (validate) return;
    EXPECT(status >= 0 ? buffer != NULL : buffer == NULL);
    if (status < 0 || !buffer) return;
    BUFFER_DESC seen = {0};
    CALL(BufferGetDesc(buffer, &seen));
    EXPECT(memcmp(&seen, &desc, sizeof desc) == 0);
    int32_t dimension = 0;
    CALL(GetType(buffer, &dimension));
    EXPECT(dimension == D3D11_RESOURCE_DIMENSION_BUFFER);
    void *parent = NULL;
    CALL(GetDevice(buffer, &parent));
    EXPECT(parent == game->device);
    if (parent) CALL(Release(parent));
    COUNT(MADE_RESOURCES);
    *slot = (Resource){buffer, 0, desc, {0}, 0, 0, 1, buffer};
}

static void create_texture(Game *game, Resource *slot) {
    Random *random = &game->random;
    const uint32_t *format = FORMATS[below(random, 4)];
    TEXTURE2D_DESC desc = {1 + below(random, 64), 1 + below(random, 64), 0, 1 + below(random, 3), format[0], {1, 0},
                           D3D11_USAGE_DEFAULT, D3D11_BIND_SHADER_RESOURCE, 0, 0};
    desc.MipLevels = below(random, full_chain(desc.Width, desc.Height) + 1);
    switch (below(random, 3)) {
    case 0: break;
    case 1: desc.Usage = D3D11_USAGE_DYNAMIC; desc.CPUAccessFlags = D3D11_CPU_ACCESS_WRITE; desc.ArraySize = 1; desc.MipLevels = 1; break;
    default: desc.Usage = D3D11_USAGE_STAGING; desc.BindFlags = 0; desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ | D3D11_CPU_ACCESS_WRITE;
    }
    TEXTURE2D_DESC resolved = desc;
    if (!resolved.MipLevels) resolved.MipLevels = full_chain(desc.Width, desc.Height);
    uint32_t count = resolved.MipLevels * desc.ArraySize;
    SUBRESOURCE_DATA *initial = NULL;
    if (below(random, 2)) {
        initial = calloc(count, sizeof *initial);
        for (uint32_t at = 0; at < count; at++) {
            uint32_t rows, row_bytes;
            subresource_rows(&resolved, at, format[1], format[2], &rows, &row_bytes);
            uint32_t pitch = row_bytes + below(random, 32);
            size_t extent = (size_t)pitch * (rows - 1) + row_bytes;
            if (extent > READ_ONLY_BYTES) extent = READ_ONLY_BYTES;
            initial[at] = (SUBRESOURCE_DATA){read_only + below(random, (uint32_t)(READ_ONLY_BYTES - extent) + 1), pitch, 0};
        }
    }
    void *texture = (void *)1;
    HRESULT status = CALL(CreateTexture2D(game->device, &desc, initial, &texture));
    free(initial);
    EXPECT(status >= 0 ? texture != NULL : texture == NULL);
    if (status < 0 || !texture) return;
    TEXTURE2D_DESC seen = {0};
    CALL(Texture2DGetDesc(texture, &seen));
    EXPECT(memcmp(&seen, &resolved, sizeof resolved) == 0);
    int32_t dimension = 0;
    CALL(GetType(texture, &dimension));
    EXPECT(dimension == D3D11_RESOURCE_DIMENSION_TEXTURE2D);
    void *surface = NULL;
    if (CALL(QueryInterface(texture, &IID_IDXGISurface, &surface)) == S_OK && surface) {
        SURFACE_DESC surface_desc = {0};
        EXPECT(CALL(SurfaceGetDesc(surface, &surface_desc)) == S_OK && surface_desc.Width == desc.Width &&
               surface_desc.Height == desc.Height && surface_desc.Format == desc.Format);
        void *unknown = NULL;
        EXPECT(CALL(QueryInterface(surface, &IID_IUnknown, &unknown)) == S_OK && unknown == texture);
        if (unknown) CALL(Release(unknown));
        CALL(Release(surface));
    } else {
        violated(__LINE__, "a texture is not a surface");
    }
    COUNT(MADE_RESOURCES);
    *slot = (Resource){texture, 1, {0}, resolved, format[1], format[2], count, texture};
}

static Resource *some_resource(Game *game) {
    Resource *slot = &game->resources[below(&game->random, OWN)];
    return slot->object ? slot : NULL;
}

/* Map, write every byte the game is told it may touch, read them back, Unmap. */
static void map_and_write(Game *game) {
    Resource *resource = some_resource(game);
    if (!resource) return;
    uint32_t subresource = below(&game->random, resource->subresources);
    int32_t type = 1 + (int32_t)below(&game->random, 5);
    MAPPED_SUBRESOURCE mapped = {(void *)1, 7, 7};
    HRESULT status = CALL(Map(game->context, resource->object, subresource, type, 0, &mapped));
    if (status < 0) {
        EXPECT(mapped.pData == NULL);
        return;
    }
    EXPECT(mapped.pData != NULL);
    if (!mapped.pData) return;
    uint8_t *bytes = mapped.pData, value = (uint8_t)next(&game->random);
    memset(bytes, value, mapped.DepthPitch);
    memset(bytes, value, mapped.RowPitch);
    uint64_t sum = 0;
    if (resource->kind == 0) {
        memset(bytes, value, resource->buffer.ByteWidth);
        for (uint32_t at = 0; at < resource->buffer.ByteWidth; at++) sum += bytes[at];
    } else {
        uint32_t rows, row_bytes;
        subresource_rows(&resource->texture, subresource, resource->block_bytes, resource->side, &rows, &row_bytes);
        for (uint32_t row = 0; row < rows; row++) {
            memset(bytes + (size_t)row * mapped.RowPitch, value, row_bytes);
            sum += bytes[(size_t)row * mapped.RowPitch + row_bytes - 1];
        }
    }
    (void)sum;
    COUNT(WROTE_MAPPINGS);
    CALL(Unmap(game->context, resource->object, subresource));
}

static void create_view(Game *game, void **slot) {
    Resource *resource = some_resource(game);
    if (!resource) return;
    void *view = (void *)1;
    HRESULT status;
    switch (below(&game->random, 3)) {
    case 0: status = CALL(CreateRenderTargetView(game->device, resource->object, NULL, &view)); break;
    case 1: status = CALL(CreateDepthStencilView(game->device, resource->object, NULL, &view)); break;
    default: {
        VIEW_DESC desc = {28, D3D11_UAV_DIMENSION_TEXTURE2D, {0, 0, 0}};
        status = CALL(CreateUnorderedAccessView(game->device, resource->object, below(&game->random, 2) ? &desc : NULL, &view));
    }
    }
    EXPECT(status >= 0 ? view != NULL : view == NULL);
    if (status < 0 || !view) return;
    void *parent = NULL;
    CALL(GetDevice(view, &parent));
    EXPECT(parent == game->device);
    if (parent) CALL(Release(parent));
    COUNT(MADE_VIEWS);
    *slot = view;
}

static uint32_t header_data_size(int32_t query) { return query == D3D11_QUERY_EVENT ? 4 : 88; }

static void create_query(Game *game, OwnQuery *slot) {
    QUERY_DESC desc = {below(&game->random, 2) ? D3D11_QUERY_EVENT : D3D11_QUERY_PIPELINE_STATISTICS, 0};
    void *query = (void *)1;
    HRESULT status = CALL(CreateQuery(game->device, &desc, &query));
    EXPECT(status >= 0 ? query != NULL : query == NULL);
    if (status < 0 || !query) return;
    EXPECT(CALL(GetDataSize(query)) == header_data_size(desc.Query));
    *slot = (OwnQuery){query, desc};
}

static void run_query(Game *game) {
    OwnQuery *query = &game->queries[below(&game->random, OWN)];
    if (!query->object) return;
    CALL(Begin(game->context, query->object));
    CALL(Draw(game->context, 3, 0));
    CALL(End(game->context, query->object));
    uint8_t data[128];
    uint32_t size = CALL(GetDataSize(query->object));
    EXPECT(size == header_data_size(query->desc.Query));
    if (size > sizeof data) return;
    if (CALL(GetData(game->context, query->object, below(&game->random, 4) ? data : NULL, below(&game->random, 4) ? size : 0, 0)) == S_OK)
        COUNT(READ_RESULTS);
}

static void bind(Game *game) {
    void *buffers[32] = {0}, *targets[8] = {0}, *views[64] = {0};
    uint32_t strides[32] = {0}, offsets[32] = {0}, counts[64];
    memset(counts, 0xff, sizeof counts);
    uint32_t start = below(&game->random, 32), count = below(&game->random, 33 - start);
    for (uint32_t at = 0; at < count; at++) {
        Resource *resource = some_resource(game);
        buffers[at] = resource && resource->kind == 0 ? resource->object : NULL;
        strides[at] = 16;
    }
    CALL(IASetVertexBuffers(game->context, start, count, buffers, strides, offsets));
    uint32_t target_count = below(&game->random, 9), view_count = below(&game->random, 9);
    for (uint32_t at = 0; at < target_count; at++) targets[at] = game->views[below(&game->random, OWN)];
    for (uint32_t at = 0; at < view_count; at++) views[at] = game->views[below(&game->random, OWN)];
    int keep = below(&game->random, 4);
    CALL(OMSetRenderTargetsAndUnorderedAccessViews(
        game->context, keep == 1 ? D3D11_KEEP_RENDER_TARGETS_AND_DEPTH_STENCIL : target_count, targets,
        game->views[below(&game->random, OWN)], target_count, keep == 2 ? D3D11_KEEP_UNORDERED_ACCESS_VIEWS : view_count,
        views, counts));
    CALL(Draw(game->context, 3 * below(&game->random, 10), 0));
}

static void identities(Game *game) {
    void *object = NULL;
    switch (below(&game->random, 3)) {
    case 0: {
        Resource *resource = some_resource(game);
        object = resource ? resource->object : NULL;
        break;
    }
    case 1: object = game->views[below(&game->random, OWN)]; break;
    default: object = game->queries[below(&game->random, OWN)].object;
    }
    if (!object) return;
    void *answer = (void *)1;
    const GUID *iid = IIDS[below(&game->random, IID_COUNT)];
    HRESULT status = CALL(QueryInterface(object, iid, &answer));
    EXPECT(status >= 0 ? answer != NULL : answer == NULL);
    if (answer) {
        void *unknown = NULL;
        EXPECT(CALL(QueryInterface(answer, &IID_IUnknown, &unknown)) == S_OK && unknown == object);
        if (unknown) CALL(Release(unknown));
        CALL(Release(answer));
    }
    CALL(AddRef(object));
    CALL(Release(object));
    void *context = NULL;
    CALL(GetImmediateContext(game->device, &context));
    EXPECT(context == game->context);
    if (context) CALL(Release(context));
    void *parent = NULL;
    CALL(GetDevice(game->context, &parent));
    EXPECT(parent == game->device);
    if (parent) CALL(Release(parent));
}

static void private_data(Game *game) {
    Resource *resource = some_resource(game);
    if (!resource) return;
    static const GUID keys[2] = {{0xf022, 1, 2, {3, 3, 3, 3, 3, 3, 3, 3}}, {0xf023, 1, 2, {3, 3, 3, 3, 3, 3, 3, 3}}};
    Kept *kept = &kept_objects[below(&game->random, 64)];
    /* Its release, inside a later call on this child or at the child's end, calls the child back. */
    atomic_store(&kept->call_back_on, below(&game->random, 4) == 0 ? resource->object : NULL);
    CALL(SetPrivateDataInterface(resource->object, &keys[below(&game->random, 2)], below(&game->random, 3) ? (void *)kept : NULL));
}

static void back_buffers(Game *game) {
    if (!game->swap_chain) return;
    uint32_t index = below(&game->random, game->back_buffers + 1);
    void *first = (void *)1, *second = (void *)1;
    const GUID *iid = below(&game->random, 2) ? &IID_ID3D11Texture2D : &IID_IDXGISurface;
    HRESULT status = CALL(GetBuffer(game->swap_chain, index, iid, &first));
    EXPECT(status >= 0 ? first != NULL : first == NULL);
    if (index >= game->back_buffers) EXPECT(status < 0);
    if (first) {
        COUNT(GOT_BACK_BUFFERS);
        HRESULT again = CALL(GetBuffer(game->swap_chain, index, iid, &second));
        /* While held, the same back buffer. */
        EXPECT(again >= 0 && second == first);
        if (second && second != (void *)1) CALL(Release(second));
        CALL(Release(first));
    }
    CALL(Present(game->swap_chain, below(&game->random, 5), 0));
}

static void *play_correct(void *argument) {
    Game *game = argument;
    while (thread_calls < game->budget) {
        if (!game->device && !make_device(game)) continue;
        switch (below(&game->random, 20)) {
        case 0: case 1: {
            Resource *slot = &game->resources[below(&game->random, OWN)];
            release_slot(&slot->object);
            if (below(&game->random, 2)) create_buffer(game, slot);
            else create_texture(game, slot);
            break;
        }
        case 2: {
            void **slot = &game->views[below(&game->random, OWN)];
            release_slot(slot);
            create_view(game, slot);
            break;
        }
        case 3: {
            OwnQuery *slot = &game->queries[below(&game->random, OWN)];
            release_slot(&slot->object);
            create_query(game, slot);
            break;
        }
        case 4: case 5: case 6: map_and_write(game); break;
        case 7: case 8: bind(game); break;
        case 9: run_query(game); break;
        case 10: case 11: identities(game); break;
        case 12: private_data(game); break;
        case 13: back_buffers(game); break;
        case 14: {
            void **view = &game->views[below(&game->random, OWN)];
            release_slot(view);
            break;
        }
        case 15: release_slot(&game->queries[below(&game->random, OWN)].object); break;
        case 16:
            if (below(&game->random, 50) == 0) {
                /* Start again on a new device. */
                for (int at = 0; at < OWN; at++) {
                    release_slot(&game->resources[at].object);
                    release_slot(&game->views[at]);
                    release_slot(&game->queries[at].object);
                }
                release_slot(&game->swap_chain);
                release_slot(&game->context);
                release_slot(&game->device);
            }
            break;
        default: CALL(Draw(game->context, 3, 0));
        }
    }
    for (int at = 0; at < OWN; at++) {
        release_slot(&game->resources[at].object);
        release_slot(&game->views[at]);
        release_slot(&game->queries[at].object);
    }
    release_slot(&game->swap_chain);
    release_slot(&game->context);
    release_slot(&game->device);
    return NULL;
}

unsigned long long fuzz_correct(CreateDevice create, CreateDeviceAndSwapChain create_swap, uint64_t seed, uint64_t calls,
                                uint32_t threads) {
    make_read_only();
    make_kept();
    pthread_t workers[32];
    Game *games = calloc(threads, sizeof *games);
    if (threads > 32) threads = 32;
    uint64_t before = atomic_load(&fuzz_calls);
    for (uint32_t at = 0; at < threads; at++) {
        games[at].random.state = (seed ^ (0x9e3779b97f4a7c15ull * (at + 1))) | 1;
        games[at].create = create;
        games[at].create_swap = create_swap;
        games[at].budget = calls / threads;
        pthread_create(&workers[at], NULL, play_correct, &games[at]);
    }
    for (uint32_t at = 0; at < threads; at++) pthread_join(workers[at], NULL);
    free(games);
    return atomic_load(&fuzz_calls) - before;
}

/* ---- The hostile game. */

enum { POOL = 512, STRIPES = 64 };

/* Every pointer the hostile game has met or made up, shared by its threads. */
static void *_Atomic pool[POOL];
/* Map, the writes into mapped memory, and Unmap of one resource pointer are serialized, as the documentation asks of
   a program: the one rule about mapped memory the glue cannot check. */
static pthread_mutex_t stripes[STRIPES];
/* A table of entry points of each kind, copied from live objects: they stay valid, as the glue's tables are static. */
enum { DEVICE, CONTEXT, BUFFER, TEXTURE, VIEW, QUERY, SWAP_CHAIN, SURFACE, KINDS };
static Entry *_Atomic tables[KINDS];
static CreateDevice hostile_create;
static CreateDeviceAndSwapChain hostile_create_swap;
static uint64_t hostile_budget;

/* Recent objects of each kind, so that many calls reach a live object of the right kind and get past the first
   check; the rest go to any pointer. */
static void *_Atomic recent[KINDS][16];

static pthread_mutex_t *stripe_of(void *resource) { return &stripes[((uintptr_t)resource >> 3) % STRIPES]; }

static void remember(int kind, void *pointer, Random *random) {
    if (pointer) atomic_store(&recent[kind][below(random, 16)], pointer);
}

static void meet(void *pointer, Random *random) {
    if (pointer) atomic_store(&pool[below(random, POOL)], pointer);
}

/* A pointer: mostly one met, else null, a misaligned or neighbouring one, a small or unmapped address. */
static void *any_pointer(Random *random) {
    void *met = atomic_load(&pool[below(random, POOL)]);
    switch (below(random, 16)) {
    case 0: return NULL;
    case 1: return (void *)(uintptr_t)0x10;
    case 2: return (void *)(uintptr_t)0xdead0000beef0000ull;
    case 3: return met ? (char *)met + 1 + below(random, 7) : NULL;
    case 4: return met ? (char *)met + 8 * (int)(below(random, 9) - 4) : NULL;
    case 5: return (void *)(uintptr_t)next(random);
    default: return met;
    }
}

/* An IUnknown* the rules allow: null, the game's own, or one of ours met before, live or released. */
static void *any_unknown(Random *random) {
    switch (below(random, 3)) {
    case 0: return NULL;
    case 1: return &kept_objects[below(random, 64)];
    default: {
        /* Only pointers met as objects: words of ours hold a table for life. */
        Entry *table = atomic_load(&tables[below(random, KINDS)]);
        (void)table;
        return atomic_load(&pool[below(random, POOL)]);
    }
    }
}

static void *any_pointer(Random *random);

/* An object of the kind, mostly a recent one, else any pointer. */
static void *any_of(int kind, Random *random) {
    void *object = below(random, 4) ? atomic_load(&recent[kind][below(random, 16)]) : NULL;
    return object ? object : any_pointer(random);
}

static void *any_resource(Random *random) { return any_of(below(random, 2) ? BUFFER : TEXTURE, random); }

static uint32_t any_count(Random *random) {
    static const uint32_t edges[] = {0, 1, 2, 7, 8, 9, 15, 16, 17, 31, 32, 33, 63, 64, 65, 0xffffffffu, 0x80000000u};
    return below(random, 3) ? edges[below(random, sizeof edges / sizeof *edges)] : (uint32_t)next(random);
}

static int32_t any_enum(Random *random) { return below(random, 2) ? (int32_t)below(random, 8) - 1 : (int32_t)next(random); }

static uint32_t any_flags(Random *random) {
    switch (below(random, 4)) {
    case 0: return 0;
    case 1: return 1u << below(random, 32);
    default: return (uint32_t)next(random);
    }
}

/* A dimension: in range and small, at the edges, or far out of range. */
static uint32_t any_side(Random *random) {
    static const uint32_t sides[] = {0, 1, 2, 3, 16, 64, 16385, 0x80000000u, 0xffffffffu};
    return below(random, 2) ? 1 + below(random, 64) : sides[below(random, sizeof sides / sizeof *sides)];
}

/* A buffer width: small, or out of range; never a valid width in the gigabytes, which would only use memory. */
static uint32_t any_width(Random *random) {
    static const uint32_t widths[] = {0, 1, 16, 64, 4096, 65536, 0x80000001u, 0xffffffffu};
    return below(random, 2) ? 1 + below(random, 4096) : widths[below(random, sizeof widths / sizeof *widths)];
}

typedef struct {
    Random random;
    uint8_t scratch[8192];
    void *arrays[64];
    uint32_t numbers[64];
} Hostile;

static void fill_arrays(Hostile *hostile, int kind) {
    for (int at = 0; at < 64; at++) {
        hostile->arrays[at] = any_of(kind, &hostile->random);
        hostile->numbers[at] = any_count(&hostile->random);
    }
}

/* An out-parameter: a valid place, or null. */
static void *out_place(Hostile *hostile) { return below(&hostile->random, 8) ? hostile->scratch : NULL; }

/* Writes into memory just mapped, within what the game was told, under the resource's stripe. */
static void write_mapped(const MAPPED_SUBRESOURCE *mapped, uint8_t value) {
    if (!mapped->pData) return;
    memset(mapped->pData, value, mapped->RowPitch);
    memset(mapped->pData, value, mapped->DepthPitch);
}

static void hostile_create_device(Hostile *hostile) {
    Random *random = &hostile->random;
    int32_t levels[16];
    for (int at = 0; at < 16; at++) levels[at] = below(random, 2) ? D3D_FEATURE_LEVEL_11_0 : (int32_t)next(random);
    uint32_t count = below(random, 2) ? below(random, 17) : any_count(random);
    void *device = NULL, *context = NULL, *swap_chain = NULL;
    int32_t level;
    if (below(random, 2)) {
        SWAP_CHAIN_DESC desc = {{any_side(random), any_side(random), 60, 1, below(random, 2) ? 28 : (uint32_t)next(random), 0, 0},
                                {below(random, 3), 0}, any_flags(random), any_count(random), NULL, 1, any_enum(random), any_flags(random)};
        if (below(random, 2))
            desc = (SWAP_CHAIN_DESC){{1 + below(random, 64), 1 + below(random, 64), 60, 1, 28, 0, 0}, {1, 0},
                                     DXGI_USAGE_RENDER_TARGET_OUTPUT, 1 + below(random, 3), NULL, 1, 0, 0};
        CALL(hostile_create_swap(any_unknown(random), any_enum(random), NULL, any_flags(random), below(random, 8) ? levels : NULL,
                                 count, below(random, 8) ? D3D11_SDK_VERSION : (uint32_t)next(random), below(random, 8) ? &desc : NULL,
                                 below(random, 2) ? &swap_chain : NULL, below(random, 8) ? &device : NULL,
                                 below(random, 4) ? &level : NULL, below(random, 8) ? &context : NULL));
    } else {
        CALL(hostile_create(any_unknown(random), below(random, 2) ? D3D_DRIVER_TYPE_HARDWARE : any_enum(random), NULL,
                            any_flags(random), below(random, 8) ? levels : NULL, count,
                            below(random, 8) ? D3D11_SDK_VERSION : (uint32_t)next(random), below(random, 8) ? &device : NULL,
                            below(random, 4) ? &level : NULL, below(random, 8) ? &context : NULL));
    }
    if (device || context) COUNT(MADE_DEVICES);
    meet(device, random);
    meet(context, random);
    meet(swap_chain, random);
    remember(DEVICE, device, random);
    remember(CONTEXT, context, random);
    remember(SWAP_CHAIN, swap_chain, random);
    if (device) atomic_store(&tables[DEVICE], *(Entry **)device);
    if (context) atomic_store(&tables[CONTEXT], *(Entry **)context);
    if (swap_chain) atomic_store(&tables[SWAP_CHAIN], *(Entry **)swap_chain);
}

/* A plausible sequence amid the chaos: a recent context's device makes a mappable resource, the context maps it
   (perhaps a subresource too far, perhaps with the wrong map type), the game writes it and maybe unmaps it, and
   maybe releases it at once, mapped. Other threads meanwhile call anything on it. */
static void hostile_map_sequence(Hostile *hostile) {
    Random *random = &hostile->random;
    Entry *context_table = atomic_load(&tables[CONTEXT]), *device_table = atomic_load(&tables[DEVICE]);
    void *context = atomic_load(&recent[CONTEXT][below(random, 16)]);
    if (!context_table || !device_table || !context) return;
    void *device = NULL;
    CALL(((GetObjectFunction)context_table[3])(context, &device));
    if (!device) return;
    void *resource = NULL;
    int staging = below(random, 2), texture = below(random, 2);
    uint32_t subresources = 1;
    if (texture) {
        TEXTURE2D_DESC desc = {1 + below(random, 64), 1 + below(random, 64), below(random, 3), 1, FORMATS[below(random, 4)][0], {1, 0},
                               staging ? D3D11_USAGE_STAGING : D3D11_USAGE_DYNAMIC, staging ? 0 : D3D11_BIND_SHADER_RESOURCE,
                               staging ? D3D11_CPU_ACCESS_READ | D3D11_CPU_ACCESS_WRITE : D3D11_CPU_ACCESS_WRITE, 0};
        if (!staging) desc.MipLevels = 1;
        subresources = desc.MipLevels ? desc.MipLevels : full_chain(desc.Width, desc.Height);
        CALL(((CreateTexture2DFunction)device_table[5])(device, &desc, NULL, &resource));
    } else {
        BUFFER_DESC desc = {1 + below(random, 4096), staging ? D3D11_USAGE_STAGING : D3D11_USAGE_DYNAMIC, staging ? 0 : D3D11_BIND_VERTEX_BUFFER,
                            staging ? D3D11_CPU_ACCESS_READ | D3D11_CPU_ACCESS_WRITE : D3D11_CPU_ACCESS_WRITE, 0, 0};
        CALL(((CreateBufferFunction)device_table[3])(device, &desc, NULL, &resource));
    }
    CALL(((CountFunction)(*(Entry **)device)[2])(device));
    if (!resource) return;
    remember(texture ? TEXTURE : BUFFER, resource, random);
    meet(resource, random);
    uint32_t subresource = below(random, subresources + 1);
    int32_t type = staging ? 1 + (int32_t)below(random, 3) : 4 + (int32_t)below(random, 2);
    if (below(random, 8) == 0) type = 1 + (int32_t)below(random, 5);
    MAPPED_SUBRESOURCE mapped = {0};
    pthread_mutex_lock(stripe_of(resource));
    if (CALL(((MapFunction)context_table[14])(context, resource, subresource, type, 0, &mapped)) == S_OK) {
        COUNT(WROTE_MAPPINGS);
        write_mapped(&mapped, (uint8_t)next(random));
        if (below(random, 8)) CALL(((UnmapFunction)context_table[15])(context, resource, subresource));
    }
    pthread_mutex_unlock(stripe_of(resource));
    if (below(random, 2)) CALL(((CountFunction)(*(Entry **)resource)[2])(resource));
}

/* One random call of a random entry of a random kind's table on a random object. */
static void hostile_call(Hostile *hostile) {
    Random *random = &hostile->random;
    int kind = (int)below(random, KINDS);
    Entry *table = atomic_load(&tables[kind]);
    if (!table) {
        /* No object of the kind met yet: make some, by the device's entries. */
        kind = DEVICE;
        table = atomic_load(&tables[DEVICE]);
    }
    if (!table || below(random, 2000) == 0) {
        hostile_create_device(hostile);
        return;
    }
    if (below(random, 16) == 0) {
        hostile_map_sequence(hostile);
        return;
    }
    void *this = any_of(kind, random);
    void *made = NULL;
    fill_arrays(hostile, below(random, 2) ? BUFFER : VIEW);
    /* The common IUnknown and ID3D11DeviceChild entries, then each kind's own. */
    uint32_t pick = below(random, 16);
    if (pick < 3) {
        if (pick == 0) {
            GUID random_iid = {(uint32_t)next(random), 0, 0, {0}};
            const GUID *iid = below(random, 8) ? IIDS[below(random, IID_COUNT)] : below(random, 2) ? &random_iid : NULL;
            CALL(((QueryInterfaceFunction)table[0])(this, iid, below(random, 8) ? (void **)&made : NULL));
        } else if (pick == 1) {
            CALL(((CountFunction)table[1])(this));
        } else {
            CALL(((CountFunction)table[2])(this));
        }
        meet(made, random);
        return;
    }
    if (kind != DEVICE && kind != SWAP_CHAIN && kind != SURFACE && pick < 6) {
        if (pick < 5) {
            CALL(((GetObjectFunction)table[3])(this, below(random, 8) ? (void **)&made : NULL));
        } else {
            GUID key = {(uint32_t)below(random, 4), 1, 2, {0}};
            CALL(((SetPrivateDataInterfaceFunction)table[6])(this, below(random, 8) ? &key : NULL, any_unknown(random)));
        }
        meet(made, random);
        return;
    }
    switch (kind) {
    case DEVICE: {
        switch (below(random, 7)) {
        case 0: {
            BUFFER_DESC desc = {any_width(random), any_enum(random), any_flags(random), any_flags(random), any_flags(random), 0};
            if (below(random, 2)) {
                /* A valid, mappable one: dynamic for writing, or staging for both. */
                int staging = below(random, 2);
                desc = (BUFFER_DESC){1 + below(random, 4096), staging ? D3D11_USAGE_STAGING : D3D11_USAGE_DYNAMIC,
                                     staging ? 0 : D3D11_BIND_VERTEX_BUFFER,
                                     staging ? D3D11_CPU_ACCESS_READ | D3D11_CPU_ACCESS_WRITE : D3D11_CPU_ACCESS_WRITE, 0, 0};
            }
            SUBRESOURCE_DATA initial = {below(random, 4) ? read_only : NULL, any_count(random), any_count(random)};
            if (desc.ByteWidth > READ_ONLY_BYTES) initial.pSysMem = NULL;
            CALL(((CreateBufferFunction)table[3])(this, below(random, 16) ? &desc : NULL, below(random, 2) ? &initial : NULL,
                                                  below(random, 8) ? &made : NULL));
            break;
        }
        case 1: {
            TEXTURE2D_DESC desc = {any_side(random), any_side(random), below(random, 2) ? below(random, 3) : any_count(random),
                                   below(random, 2) ? 1 + below(random, 2) : any_count(random),
                                   below(random, 2) ? FORMATS[below(random, 4)][0] : (uint32_t)next(random),
                                   {below(random, 8) ? 1 : any_count(random), 0}, any_enum(random), any_flags(random),
                                   any_flags(random), any_flags(random)};
            if (below(random, 2)) {
                int staging = below(random, 2);
                desc = (TEXTURE2D_DESC){1 + below(random, 64), 1 + below(random, 64), 1, 1, FORMATS[below(random, 4)][0], {1, 0},
                                        staging ? D3D11_USAGE_STAGING : D3D11_USAGE_DYNAMIC, staging ? 0 : D3D11_BIND_SHADER_RESOURCE,
                                        staging ? D3D11_CPU_ACCESS_READ | D3D11_CPU_ACCESS_WRITE : D3D11_CPU_ACCESS_WRITE, 0};
            }
            /* Initial data only for a small texture, each subresource's bytes as long as the header says it reads. */
            SUBRESOURCE_DATA initial[128];
            int small = desc.Width <= 64 && desc.Height <= 64 && desc.ArraySize <= 2 && desc.MipLevels <= 7;
            for (int at = 0; at < 128; at++) initial[at] = (SUBRESOURCE_DATA){below(random, 16) ? read_only : NULL, 4096, 0};
            CALL(((CreateTexture2DFunction)table[5])(this, below(random, 16) ? &desc : NULL, small && below(random, 2) ? initial : NULL,
                                                     below(random, 8) ? &made : NULL));
            break;
        }
        case 2: case 3: case 4: {
            VIEW_DESC desc = {below(random, 2) ? 28 : (uint32_t)next(random), any_enum(random), {any_count(random), any_count(random), any_count(random)}};
            uint8_t depth_desc[24] = {0};
            memcpy(depth_desc, &desc, sizeof desc);
            const void *pdesc = below(random, 2) ? (const void *)depth_desc : NULL;
            CALL(((CreateViewFunction)table[8 + below(random, 3)])(this, any_resource(random), pdesc, below(random, 8) ? &made : NULL));
            break;
        }
        case 5: {
            QUERY_DESC desc = {any_enum(random), below(random, 2) ? 0 : any_flags(random)};
            CALL(((CreateQueryFunction)table[24])(this, below(random, 8) ? &desc : NULL, below(random, 8) ? &made : NULL));
            break;
        }
        default: CALL(((GetObjectFunction)table[40])(this, below(random, 8) ? &made : NULL));
        }
        break;
    }
    case CONTEXT: {
        switch (below(random, 9)) {
        case 0: CALL(((DrawFunction)table[13])(this, any_count(random), any_count(random))); break;
        case 1: case 2: {
            void *resource = any_resource(random);
            uint32_t subresource = below(random, 4) ? below(random, 3) : any_count(random);
            MAPPED_SUBRESOURCE mapped = {0};
            pthread_mutex_lock(stripe_of(resource));
            HRESULT status = CALL(((MapFunction)table[14])(this, resource, subresource, below(random, 2) ? 1 + (int32_t)below(random, 5) : any_enum(random),
                                                          below(random, 4) ? 0 : any_flags(random), below(random, 16) ? &mapped : NULL));
            if (status == S_OK) {
                COUNT(WROTE_MAPPINGS);
                write_mapped(&mapped, (uint8_t)next(random));
                if (below(random, 8)) CALL(((UnmapFunction)table[15])(this, resource, subresource));
            }
            pthread_mutex_unlock(stripe_of(resource));
            break;
        }
        case 3: {
            void *resource = any_resource(random);
            pthread_mutex_lock(stripe_of(resource));
            CALL(((UnmapFunction)table[15])(this, resource, below(random, 4) ? below(random, 3) : any_count(random)));
            pthread_mutex_unlock(stripe_of(resource));
            break;
        }
        case 4:
            CALL(((SetVertexBuffersFunction)table[18])(this, any_count(random), any_count(random), below(random, 8) ? hostile->arrays : NULL,
                                                       below(random, 8) ? hostile->numbers : NULL, below(random, 8) ? hostile->numbers : NULL));
            break;
        case 5:
            CALL(((OutputMergerFunction)table[34])(this, any_count(random), below(random, 8) ? hostile->arrays : NULL, any_of(VIEW, random),
                                                   any_count(random), any_count(random), below(random, 8) ? hostile->arrays : NULL,
                                                   below(random, 8) ? hostile->numbers : NULL));
            break;
        case 6: CALL(((AsynchronousFunction)table[27])(this, any_of(QUERY, random))); break;
        case 7: CALL(((AsynchronousFunction)table[28])(this, any_of(QUERY, random))); break;
        default: {
            uint32_t size = below(random, 2) ? (below(random, 2) ? 4 : 88) : any_count(random);
            void *data = below(random, 4) && size <= sizeof hostile->scratch ? hostile->scratch : NULL;
            CALL(((GetDataFunction)table[29])(this, any_of(QUERY, random), data, size, below(random, 4) ? 0 : any_flags(random)));
        }
        }
        break;
    }
    case BUFFER: case TEXTURE: {
        if (below(random, 2)) CALL(((GetTypeFunction)table[7])(this, out_place(hostile)));
        else if (kind == BUFFER) CALL(((GetBufferDescFunction)table[10])(this, out_place(hostile)));
        else CALL(((GetTexture2DDescFunction)table[10])(this, out_place(hostile)));
        break;
    }
    case QUERY: {
        if (below(random, 2)) CALL(((GetDataSizeFunction)table[7])(this));
        else CALL(((void (*)(void *, QUERY_DESC *))table[8])(this, out_place(hostile)));
        break;
    }
    case SURFACE: CALL(((SurfaceGetDescFunction)table[8])(this, out_place(hostile))); break;
    case SWAP_CHAIN: {
        if (below(random, 2)) {
            CALL(((PresentFunction)table[8])(this, any_count(random), any_flags(random)));
        } else {
            const GUID *iid = below(random, 8) ? IIDS[below(random, IID_COUNT)] : NULL;
            CALL(((GetBufferFunction)table[9])(this, any_count(random) % 20, iid, below(random, 8) ? &made : NULL));
        }
        break;
    }
    default: break;
    }
    meet(made, random);
    if (made) COUNT(MADE_RESOURCES);
    /* Learn the tables of what was made, for later calls. */
    if (made && (kind == DEVICE || kind == SWAP_CHAIN)) {
        /* By QueryInterface for each kind: the answer's own table, which is the kind's (a back buffer may come as
           its surface, whose table is not a texture's). */
        static const struct { const GUID *iid; int kind; } kinds[] = {{&IID_ID3D11Buffer, BUFFER}, {&IID_ID3D11Texture2D, TEXTURE},
                                                                      {&IID_IDXGISurface, SURFACE}, {&IID_ID3D11Query, QUERY},
                                                                      {&IID_ID3D11View, VIEW}};
        for (size_t at = 0; at < sizeof kinds / sizeof *kinds; at++) {
            void *answer = NULL;
            if (((QueryInterfaceFunction)(*(Entry **)made)[0])(made, kinds[at].iid, &answer) == S_OK && answer) {
                atomic_store(&tables[kinds[at].kind], *(Entry **)answer);
                remember(kinds[at].kind, answer, random);
                meet(answer, random);
                ((CountFunction)(*(Entry **)answer)[2])(answer);
            }
        }
    }
    /* Sometimes give the reference back at once, sometimes keep it, sometimes over-release. */
    if (made && below(random, 4) == 0) CALL(((CountFunction)(*(Entry **)made)[2])(made));
}

static void *play_hostile(void *argument) {
    Hostile *hostile = argument;
    while (thread_calls < hostile_budget) hostile_call(hostile);
    return NULL;
}

unsigned long long fuzz_hostile(CreateDevice create, CreateDeviceAndSwapChain create_swap, uint64_t seed, uint64_t calls,
                                uint32_t threads) {
    make_read_only();
    make_kept();
    for (int at = 0; at < STRIPES; at++) pthread_mutex_init(&stripes[at], NULL);
    hostile_create = create;
    hostile_create_swap = create_swap;
    if (threads > 32) threads = 32;
    hostile_budget = calls / threads;
    Hostile *hostiles = calloc(threads, sizeof *hostiles);
    for (uint32_t at = 0; at < threads; at++) hostiles[at].random.state = (seed ^ (0xd1b54a32d192ed03ull * (at + 1))) | 1;
    for (int tries = 0; tries < 1000 && !atomic_load(&tables[CONTEXT]); tries++) hostile_create_device(&hostiles[0]);
    pthread_t workers[32];
    uint64_t before = atomic_load(&fuzz_calls);
    for (uint32_t at = 0; at < threads; at++) pthread_create(&workers[at], NULL, play_hostile, &hostiles[at]);
    for (uint32_t at = 0; at < threads; at++) pthread_join(workers[at], NULL);
    free(hostiles);
    for (int kind = 0; kind < KINDS; kind++)
        if (atomic_load(&tables[kind])) atomic_fetch_or(&fuzz_counts[REACHED_TABLES], 1ull << kind);
    return atomic_load(&fuzz_calls) - before;
}
