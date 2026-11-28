/* Exactly sized bytes across both glues, for tests/exact (compiled with AddressSanitizer in its sanitizer run).

   Two parts, each a C program as Microsoft's headers lay it out (d3d11.h's tables, called through their slots):

   - A caller of the serving glue's exports: it hands over initial data and uploads in heap blocks of exactly the
     length the test names (the reference's), and touches exactly the bytes the test names of the memory Map lends,
     so that the serving glue reading one byte past an upload, or lending one byte too few, is an AddressSanitizer
     report.
   - A stand-in system for the calling glue: d3d11.dll's device, immediate context, buffers and 1D, 2D and 3D textures,
     which read exactly the length the test names of every initial subresource and upload, and lend from Map a heap
     block of exactly the length the test names, so that the calling glue passing fewer bytes than the documentation
     makes the system read, or viewing more than it lends, is an AddressSanitizer report.

   Slots (d3d11.h, the order of each interface's methods after IUnknown's three): ID3D11Device CreateBuffer 3,
   CreateTexture1D 4, CreateTexture2D 5, CreateTexture3D 6, GetImmediateContext 40; ID3D11DeviceContext Map 14, Unmap 15,
   UpdateSubresource 48, CreateQuery 24; ID3D11DeviceChild GetDevice 3; ID3D11Asynchronous GetDataSize 7, ID3D11Query
   GetDesc 8, ID3D11Query1 GetDesc1 9; ID3D11Resource GetType 7; GetDesc 10 of each resource;
   ID3D11Multithread Enter 3, Leave 4, SetMultithreadProtected 5, GetMultithreadProtected 6. */

#include <stdatomic.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

typedef int32_t HRESULT;
typedef void *Entry;

#define S_OK 0
#define E_NOTIMPL ((HRESULT)0x80004001u)
#define E_NOINTERFACE ((HRESULT)0x80004002u)
#define E_POINTER ((HRESULT)0x80004003u)
#define E_INVALIDARG ((HRESULT)0x80070057u)

typedef struct {
    const void *pSysMem;
    uint32_t SysMemPitch, SysMemSlicePitch;
} SUBRESOURCE_DATA;

typedef struct {
    void *pData;
    uint32_t RowPitch, DepthPitch;
} MAPPED_SUBRESOURCE;

/* The entry at `index` of an object's table. */
#define SLOT(object, index, type) ((type)((*(Entry **)(object))[index]))

typedef HRESULT (*CreateDeviceFunction)(void *, int32_t, void *, uint32_t, const int32_t *, uint32_t, uint32_t, void **,
                                        int32_t *, void **);
typedef HRESULT (*CreateResourceFunction)(void *, const void *, const SUBRESOURCE_DATA *, void **);
typedef HRESULT (*MapFunction)(void *, void *, uint32_t, int32_t, uint32_t, MAPPED_SUBRESOURCE *);
typedef void (*UnmapFunction)(void *, void *, uint32_t);
typedef void (*UpdateFunction)(void *, void *, uint32_t, const uint32_t *, const void *, uint32_t, uint32_t);
typedef uint32_t (*CountFunction)(void *);

/* A heap block of exactly `length` bytes (malloc(0) for none, which AddressSanitizer reports any access to), filled. */
static uint8_t *exact_block(uint64_t length, uint32_t seed) {
    uint8_t *bytes = malloc((size_t)length);
    for (uint64_t at = 0; at < length; at++) bytes[at] = (uint8_t)(at * 31u + seed);
    return bytes;
}

/* ---- The caller of the serving glue. ---- */

HRESULT exact_create_device(uintptr_t create, void **device, void **context) {
    int32_t level = 0xb000, made = 0;
    /* D3D_DRIVER_TYPE_HARDWARE, no flags, D3D11_SDK_VERSION. */
    return ((CreateDeviceFunction)create)(NULL, 1, NULL, 0, &level, 1, 7, device, &made, context);
}

/* Creates a resource of `kind` (0 buffer, 1, 2 or 3 the texture's dimensions) from `desc`'s bytes, with `count`
   subresources of initial data, each a block of exactly `lengths[at]` bytes at its pitches (none for `count` 0). */
HRESULT exact_create(void *device, uint32_t kind, const void *desc, uint32_t count, const uint64_t *lengths,
                     const uint32_t *pitches, const uint32_t *slice_pitches, void **out) {
    SUBRESOURCE_DATA *initial = count ? calloc(count, sizeof *initial) : NULL;
    for (uint32_t at = 0; at < count; at++) {
        initial[at].pSysMem = exact_block(lengths[at], at);
        initial[at].SysMemPitch = pitches[at];
        initial[at].SysMemSlicePitch = slice_pitches[at];
    }
    HRESULT status = SLOT(device, 3 + kind, CreateResourceFunction)(device, desc, initial, out);
    for (uint32_t at = 0; at < count; at++) free((void *)initial[at].pSysMem);
    free(initial);
    return status;
}

/* UpdateSubresource from a block of exactly `length` bytes; `region` is the box's six words, or null. */
void exact_update(void *context, void *resource, uint32_t subresource, const uint32_t *region, uint64_t length,
                  uint32_t row_pitch, uint32_t depth_pitch) {
    uint8_t *bytes = exact_block(length, subresource);
    SLOT(context, 48, UpdateFunction)(context, resource, subresource, region, bytes, row_pitch, depth_pitch);
    free(bytes);
}

/* Maps, reads and writes back each of the first `touch(row pitch, depth pitch)` bytes Map lent, and unmaps; writes how
   many bytes it touched. */
HRESULT exact_map(void *context, void *resource, uint32_t subresource, int32_t map_type,
                  uint64_t (*touch)(uint32_t, uint32_t), uint64_t *touched) {
    MAPPED_SUBRESOURCE mapped = {0};
    HRESULT status = SLOT(context, 14, MapFunction)(context, resource, subresource, map_type, 0, &mapped);
    if (status < 0) return status;
    uint64_t length = touch(mapped.RowPitch, mapped.DepthPitch);
    volatile uint8_t *bytes = mapped.pData;
    for (uint64_t at = 0; at < length; at++) bytes[at] = (uint8_t)(bytes[at] ^ 0x5a);
    *touched = length;
    SLOT(context, 15, UnmapFunction)(context, resource, subresource);
    return S_OK;
}

uint32_t exact_release(void *object) { return SLOT(object, 2, CountFunction)(object); }

/* The values the calling glue's pins never pass, handed to the serving glue directly: it passes them on (the pins are
   the calling glue's, for the system). Slots: ID3D11Device SetExceptionMode 41; ID3D11InfoQueue SetBreakOnCategory 30,
   SetBreakOnSeverity 31, SetBreakOnID 32; ID3D11Device OpenSharedResource 28. */
typedef HRESULT (*SetExceptionModeFunction)(void *, uint32_t);
typedef HRESULT (*SetBreakFunction)(void *, int32_t, int32_t);

HRESULT exact_set_exception_mode(void *device, uint32_t flags) {
    return SLOT(device, 41, SetExceptionModeFunction)(device, flags);
}

/* ID3D11Device::OpenSharedResource (slot 28) of the handle `handle`, for the interface whose 16 bytes `iid` holds. */
typedef HRESULT (*OpenSharedFunction)(void *, uintptr_t, const void *, void **);
HRESULT exact_open_shared(void *device, uintptr_t handle, const void *iid, void **out) {
    return SLOT(device, 28, OpenSharedFunction)(device, handle, iid, out);
}

HRESULT exact_set_break(void *queue, uint32_t slot, int32_t value, int32_t enable) {
    return SLOT(queue, slot, SetBreakFunction)(queue, value, enable);
}

/* QueryInterface (slot 0) of `object` for the interface whose 16 bytes `iid` holds. */
typedef HRESULT (*QueryFunction)(void *, const void *, void **);
HRESULT exact_query(void *object, const void *iid, void **out) { return SLOT(object, 0, QueryFunction)(object, iid, out); }

/* DXGI's calls whose values the calling glue never passes (pinned: never called, or a module), handed to the serving
   glue directly: IDXGIDevice2 OfferResources 14 and ReclaimResources 15, IDXGIDevice4 OfferResources1 18 and
   ReclaimResources1 19 (dxgi1_2.h, dxgi1_5.h), each of the one resource `resource`; IDXGIFactory CreateSoftwareAdapter 11
   (dxgi.h). */
typedef HRESULT (*OfferFunction)(void *, uint32_t, void *const *, int32_t);
typedef HRESULT (*Offer1Function)(void *, uint32_t, void *const *, int32_t, uint32_t);
typedef HRESULT (*ReclaimFunction)(void *, uint32_t, void *const *, int32_t *);
typedef HRESULT (*SoftwareAdapterFunction)(void *, uintptr_t, void **);
HRESULT exact_offer(void *device, void *resource, int32_t priority) {
    return SLOT(device, 14, OfferFunction)(device, 1, &resource, priority);
}
HRESULT exact_reclaim(void *device, void *resource, int32_t *discarded) {
    return SLOT(device, 15, ReclaimFunction)(device, 1, &resource, discarded);
}
HRESULT exact_offer1(void *device, void *resource, int32_t priority, uint32_t flags) {
    return SLOT(device, 18, Offer1Function)(device, 1, &resource, priority, flags);
}
HRESULT exact_reclaim1(void *device, void *resource, int32_t *results) {
    return SLOT(device, 19, ReclaimFunction)(device, 1, &resource, results);
}
HRESULT exact_software_adapter(void *factory, uintptr_t module, void **adapter) {
    return SLOT(factory, 11, SoftwareAdapterFunction)(factory, module, adapter);
}

/* CreateDeferredContext of each device version (ID3D11Device 27, ID3D11Device1 44, ID3D11Device2 51, ID3D11Device3 62:
   d3d11.h, d3d11_1.h, d3d11_2.h, d3d11_3.h), which the calling glue never calls, with the flags `flags`. */
typedef HRESULT (*DeferredFunction)(void *, uint32_t, void **);
HRESULT exact_deferred(void *device, uint32_t slot, uint32_t flags, void **out) {
    return SLOT(device, slot, DeferredFunction)(device, flags, out);
}

/* D3D11CreateDevice with the software rasterizer module `module` (D3D_DRIVER_TYPE_SOFTWARE, 4). */
HRESULT exact_create_device_with_module(uintptr_t create, uintptr_t module, void **device, void **context) {
    int32_t level = 0xb000, made = 0;
    return ((CreateDeviceFunction)create)(NULL, 4, (void *)module, 0, &level, 1, 7, device, &made, context);
}

/* ---- The stand-in system. ---- */

enum {
    KIND_DEVICE,
    KIND_CONTEXT,
    KIND_MULTITHREAD,
    KIND_BUFFER,
    KIND_TEXTURE_1D,
    KIND_TEXTURE_2D,
    KIND_TEXTURE_3D,
    KIND_QUERY,
    KIND_PREDICATE,
    KINDS
};

/* Each kind's description size: D3D11_BUFFER_DESC 24, D3D11_TEXTURE1D_DESC 32, D3D11_TEXTURE2D_DESC 44,
   D3D11_TEXTURE3D_DESC 36, D3D11_QUERY_DESC 8. */
static const size_t DESC_SIZE[KINDS] = {0, 0, 0, 24, 32, 44, 36, 8, 8};
/* D3D11_RESOURCE_DIMENSION of each resource kind. */
static const int32_t DIMENSION[KINDS] = {0, 0, 0, 1, 2, 3, 4, 0, 0};

typedef struct Object {
    Entry *table;
    atomic_uint references;
    int kind;
    uint8_t desc[48];
} Object;

/* The interface ids each kind answers, as the test gives them (16 bytes each). */
static uint8_t answered[KINDS][16][16];
static size_t answered_count[KINDS];

/* What the test expects the system to read or lend next. */
static uint64_t expected[4096];
static size_t expected_count;
static uint32_t map_row_pitch, map_depth_pitch;
static uint64_t map_length;
static void *mapped_block;

/* A sum of every byte the system read, so that no read is optimized away. */
static volatile uint64_t read_sum;

/* Reads each of `length` bytes at `bytes`, one by one, as the system would. */
static void read_exactly(const void *bytes, uint64_t length) {
    const volatile uint8_t *at = bytes;
    uint64_t sum = 0;
    for (uint64_t index = 0; index < length; index++) sum += at[index];
    read_sum += sum;
}

/* What the system saw: creations, uploads, maps, unmaps, calls of a slot it does not serve, unmaps of another resource
   or subresource than the one mapped, and devices made. */
static atomic_ullong seen[7];
/* The resource and subresource the one Map lent. */
static void *mapped_resource;
static uint32_t mapped_subresource;

static Entry device_table[96], context_table[160], multithread_table[7], resource_table[16], query_table[16];
static Object device = {device_table, 1, KIND_DEVICE, {0}};
static Object context = {context_table, 1, KIND_CONTEXT, {0}};
static Object multithread = {multithread_table, 1, KIND_MULTITHREAD, {0}};

void exact_system_answer(uint32_t kind, const uint8_t *iids, size_t count) {
    if (kind >= KINDS || count > 16) return;
    memcpy(answered[kind], iids, count * 16);
    answered_count[kind] = count;
}

void exact_system_expect(const uint64_t *lengths, size_t count) {
    if (count > sizeof expected / sizeof *expected) count = 0;
    memcpy(expected, lengths, count * sizeof *lengths);
    expected_count = count;
}

void exact_system_map_with(uint32_t row_pitch, uint32_t depth_pitch, uint64_t length) {
    map_row_pitch = row_pitch;
    map_depth_pitch = depth_pitch;
    map_length = length;
}

void exact_system_seen(uint64_t *out) {
    for (int at = 0; at < 7; at++) out[at] = atomic_load(&seen[at]);
}

static intptr_t not_served(void) {
    atomic_fetch_add(&seen[4], 1);
    return E_NOTIMPL;
}

static uint32_t add_ref(void *this) { return atomic_fetch_add(&((Object *)this)->references, 1) + 1; }

static uint32_t release(void *this) {
    Object *object = this;
    uint32_t left = atomic_fetch_sub(&object->references, 1) - 1;
    if (left == 0 && object->kind >= KIND_BUFFER) free(object);
    return left;
}

static HRESULT query_interface(void *this, const uint8_t *iid, void **out) {
    if (!out) return E_POINTER;
    Object *object = this;
    *out = NULL;
    if (object->kind == KIND_CONTEXT) {
        for (size_t at = 0; at < answered_count[KIND_MULTITHREAD]; at++)
            if (memcmp(iid, answered[KIND_MULTITHREAD][at], 16) == 0) {
                add_ref(&multithread);
                *out = &multithread;
                return S_OK;
            }
    }
    for (size_t at = 0; at < answered_count[object->kind]; at++)
        if (memcmp(iid, answered[object->kind][at], 16) == 0) {
            add_ref(object);
            *out = object;
            return S_OK;
        }
    return E_NOINTERFACE;
}

/* Reads exactly the expected length of each initial subresource the test named. */
static HRESULT create(int kind, const void *desc, const SUBRESOURCE_DATA *initial, void **out) {
    if (!desc || !out) return E_INVALIDARG;
    for (size_t at = 0; initial && at < expected_count; at++) read_exactly(initial[at].pSysMem, expected[at]);
    Object *object = calloc(1, sizeof *object);
    object->table = resource_table;
    object->references = 1;
    object->kind = kind;
    memcpy(object->desc, desc, DESC_SIZE[kind]);
    atomic_fetch_add(&seen[0], 1);
    *out = object;
    return S_OK;
}

/* CreateQuery: an object of the predicate kind for a predicate's description (D3D11_QUERY_OCCLUSION_PREDICATE,
   D3D11_QUERY_SO_OVERFLOW_PREDICATE and its four streams), of the query kind for any other; each answers the interface
   ids the test gave its kind. */
static HRESULT create_query(void *this, const int32_t *desc, void **out) {
    (void)this;
    if (!desc || !out) return E_INVALIDARG;
    int predicate = desc[0] == 5 || desc[0] == 7 || (desc[0] >= 12 && desc[0] <= 15);
    HRESULT status = create(predicate ? KIND_PREDICATE : KIND_QUERY, desc, NULL, out);
    if (status == S_OK) ((Object *)*out)->table = query_table;
    return status;
}

static uint32_t get_data_size(void *this) {
    (void)this;
    return 8;
}

static HRESULT create_buffer(void *this, const void *desc, const SUBRESOURCE_DATA *initial, void **out) {
    (void)this;
    return create(KIND_BUFFER, desc, initial, out);
}
static HRESULT create_texture_1d(void *this, const void *desc, const SUBRESOURCE_DATA *initial, void **out) {
    (void)this;
    return create(KIND_TEXTURE_1D, desc, initial, out);
}
static HRESULT create_texture_2d(void *this, const void *desc, const SUBRESOURCE_DATA *initial, void **out) {
    (void)this;
    return create(KIND_TEXTURE_2D, desc, initial, out);
}
static HRESULT create_texture_3d(void *this, const void *desc, const SUBRESOURCE_DATA *initial, void **out) {
    (void)this;
    return create(KIND_TEXTURE_3D, desc, initial, out);
}

static void get_immediate_context(void *this, void **out) {
    (void)this;
    add_ref(&context);
    *out = &context;
}

static void get_device(void *this, void **out) {
    (void)this;
    add_ref(&device);
    *out = &device;
}

static void get_type(void *this, int32_t *out) { *out = DIMENSION[((Object *)this)->kind]; }

static void get_desc(void *this, void *out) {
    Object *object = this;
    memcpy(out, object->desc, DESC_SIZE[object->kind]);
}

/* Lends a heap block of exactly the length the test named, at the pitches it named. */
static HRESULT map(void *this, void *resource, uint32_t subresource, int32_t type, uint32_t flags,
                   MAPPED_SUBRESOURCE *out) {
    (void)this, (void)type, (void)flags;
    if (!out || mapped_block) return E_INVALIDARG;
    mapped_block = exact_block(map_length, 3);
    mapped_resource = resource;
    mapped_subresource = subresource;
    out->pData = mapped_block;
    out->RowPitch = map_row_pitch;
    out->DepthPitch = map_depth_pitch;
    atomic_fetch_add(&seen[2], 1);
    return S_OK;
}

static void unmap(void *this, void *resource, uint32_t subresource) {
    (void)this;
    if (resource != mapped_resource || subresource != mapped_subresource) atomic_fetch_add(&seen[5], 1);
    free(mapped_block);
    mapped_block = NULL;
    atomic_fetch_add(&seen[3], 1);
}

/* Reads exactly the expected length (the first the test named) of the upload. */
static void update_subresource(void *this, void *resource, uint32_t subresource, const uint32_t *region,
                               const void *bytes, uint32_t row_pitch, uint32_t depth_pitch) {
    (void)this, (void)resource, (void)subresource, (void)region, (void)row_pitch, (void)depth_pitch;
    read_exactly(bytes, expected_count ? expected[0] : 0);
    atomic_fetch_add(&seen[1], 1);
}

static void multithread_enter(void *this) { (void)this; }
static void multithread_leave(void *this) { (void)this; }
static int32_t multithread_set(void *this, int32_t on) {
    (void)this, (void)on;
    return 1;
}
static int32_t multithread_get(void *this) {
    (void)this;
    return 1;
}

static void fill(Entry *table, size_t count) {
    for (size_t at = 0; at < count; at++) table[at] = (Entry)not_served;
    table[0] = (Entry)query_interface;
    table[1] = (Entry)add_ref;
    table[2] = (Entry)release;
}

static void tables(void) {
    fill(device_table, sizeof device_table / sizeof *device_table);
    device_table[3] = (Entry)create_buffer;
    device_table[4] = (Entry)create_texture_1d;
    device_table[5] = (Entry)create_texture_2d;
    device_table[6] = (Entry)create_texture_3d;
    device_table[40] = (Entry)get_immediate_context;
    fill(context_table, sizeof context_table / sizeof *context_table);
    context_table[3] = (Entry)get_device;
    context_table[14] = (Entry)map;
    context_table[15] = (Entry)unmap;
    context_table[48] = (Entry)update_subresource;
    fill(multithread_table, 7);
    multithread_table[3] = (Entry)multithread_enter;
    multithread_table[4] = (Entry)multithread_leave;
    multithread_table[5] = (Entry)multithread_set;
    multithread_table[6] = (Entry)multithread_get;
    device_table[24] = (Entry)create_query;
    fill(query_table, sizeof query_table / sizeof *query_table);
    query_table[3] = (Entry)get_device;
    query_table[7] = (Entry)get_data_size;
    query_table[8] = (Entry)get_desc;
    query_table[9] = (Entry)get_desc;
    fill(resource_table, sizeof resource_table / sizeof *resource_table);
    resource_table[3] = (Entry)get_device;
    resource_table[7] = (Entry)get_type;
    resource_table[10] = (Entry)get_desc;
}

/* D3D11CreateDevice of the stand-in: the one device and its immediate context, at the first level asked for. */
HRESULT exact_system_create_device(void *adapter, int32_t driver_type, void *software, uint32_t flags,
                                   const int32_t *levels, uint32_t level_count, uint32_t sdk_version, void **out_device,
                                   int32_t *level, void **out_context) {
    (void)adapter, (void)driver_type, (void)software, (void)flags, (void)sdk_version;
    if (!device_table[0]) tables();
    atomic_fetch_add(&seen[6], 1);
    if (out_device) {
        add_ref(&device);
        *out_device = &device;
    }
    if (level) *level = level_count && levels ? levels[0] : 0xb000;
    if (out_context) {
        add_ref(&context);
        *out_context = &context;
    }
    return S_OK;
}

/* D3D11CreateDeviceAndSwapChain of the stand-in: not served. */
HRESULT exact_system_create_device_and_swap_chain(void *adapter, int32_t driver_type, void *software, uint32_t flags,
                                                  const int32_t *levels, uint32_t level_count, uint32_t sdk_version,
                                                  const void *desc, void **swap_chain, void **out_device,
                                                  int32_t *level, void **out_context) {
    (void)adapter, (void)driver_type, (void)software, (void)flags, (void)levels, (void)level_count, (void)sdk_version,
        (void)desc, (void)level, (void)out_context;
    if (swap_chain) *swap_chain = NULL;
    if (out_device) *out_device = NULL;
    return E_NOTIMPL;
}
