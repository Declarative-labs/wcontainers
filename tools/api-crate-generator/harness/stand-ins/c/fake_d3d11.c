/* A stand-in for the system's d3d11.dll on Linux: the spike's slice of Direct3D 11 as plain C COM objects, written
   from Microsoft's documentation of the interfaces. Each object is a table pointer, a reference count and its state;
   tables hold the slice's entries at their d3d11.h indices and null elsewhere. A texture is also an IDXGISurface: a
   second table pointer inside the object, as C++ multiple inheritance lays it out. The immediate context answers
   ID3D11Multithread (d3d11_4.h); once protected, a recursive lock of the device's serializes its methods, as
   Microsoft documents for the runtime. Counters and private data are guarded, since the device and its children
   are free threaded. */
#include <pthread.h>
#include <stdatomic.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#include "d3d11_slice.h"

enum { KIND_DEVICE, KIND_CONTEXT, KIND_BUFFER, KIND_QUERY, KIND_TEXTURE, KIND_RTV, KIND_DSV, KIND_UAV, KIND_SWAP_CHAIN };
enum { PRIVATE_SLOTS = 4, VERTEX_SLOTS = 32, TARGET_SLOTS = 8, VIEW_SLOTS = 64, BACK_BUFFERS = 16 };
/* The fake's row alignment, unlike the front's, so that the glue is seen to use the system's pitch. */
enum { ROW_ALIGNMENT = 32 };

struct Device;

typedef struct Header {
    Entry *table;
    atomic_uint references;
    int kind;
    struct Device *device;
    pthread_mutex_t private_lock;
    GUID keys[PRIVATE_SLOTS];
    void *values[PRIVATE_SLOTS];
} Header;

typedef struct Context {
    Header header;
    Entry *multithread; /* the ID3D11Multithread pointer is this field's address */
    pthread_mutex_t lock;
    atomic_int protected_;
    uint64_t vertices;
    void *bound[VERTEX_SLOTS];
    uint32_t strides[VERTEX_SLOTS];
    uint32_t offsets[VERTEX_SLOTS];
    void *targets[TARGET_SLOTS];
    void *depth;
    void *views[VIEW_SLOTS];
} Context;

typedef struct Device {
    Header header;
    Context *context;
} Device;

typedef struct Buffer {
    Header header;
    BUFFER_DESC desc;
    uint8_t *bytes;
    int mapped;
} Buffer;

typedef struct Texture {
    Header header;
    Entry *surface; /* the IDXGISurface pointer is this field's address */
    TEXTURE2D_DESC desc;
    uint32_t count;
    uint32_t *pitch, *rows;
    uint8_t **bytes;
    uint8_t *mapped;
} Texture;

typedef struct View {
    Header header;
    void *resource;
} View;

typedef struct Query {
    Header header;
    QUERY_DESC desc;
    int begun;
    int ended;
    uint64_t begun_at;
    uint64_t vertices;
} Query;

typedef struct SwapChain {
    Header header;
    uint32_t count;
    void *buffers[BACK_BUFFERS];
    uint32_t presents;
} SwapChain;

static atomic_long live_objects;
static Observed observed;
static pthread_mutex_t observed_lock = PTHREAD_MUTEX_INITIALIZER;

long fake_live_objects(void) { return atomic_load(&live_objects); }

void fake_observed(Observed *out) {
    pthread_mutex_lock(&observed_lock);
    *out = observed;
    pthread_mutex_unlock(&observed_lock);
}

/* check-2: when set, the context holds a reference to each bound vertex buffer, as Microsoft documents for
   IASetVertexBuffers ("The method will hold a reference to the interfaces passed in"), and releases the one a slot
   held before, inside the call. */
static atomic_int holds_bound;
void fake_set_holds_bound(int holds) { atomic_store(&holds_bound, holds); }

/* The device's recursive lock around its immediate context. The runtime takes it once ID3D11Multithread protection is
   on; the fake takes it always, and records the protection, which a test reads. */
static void enter(void *context) { pthread_mutex_lock(&((Context *)context)->lock); }

static void leave(void *context) { pthread_mutex_unlock(&((Context *)context)->lock); }


#define OBSERVE(statement)                                                                                           \
    do {                                                                                                             \
        pthread_mutex_lock(&observed_lock);                                                                          \
        statement;                                                                                                   \
        pthread_mutex_unlock(&observed_lock);                                                                        \
    } while (0)

static int surface_interface(const GUID *iid) {
    return guid_equal(iid, &IID_IDXGIObject) || guid_equal(iid, &IID_IDXGIDeviceSubObject) ||
           guid_equal(iid, &IID_IDXGISurface);
}

static int answers(int kind, const GUID *iid) {
    if (guid_equal(iid, &IID_IUnknown)) return 1;
    int child = guid_equal(iid, &IID_ID3D11DeviceChild);
    int view = child || guid_equal(iid, &IID_ID3D11View);
    switch (kind) {
    case KIND_DEVICE: return guid_equal(iid, &IID_ID3D11Device);
    case KIND_CONTEXT: return child || guid_equal(iid, &IID_ID3D11DeviceContext);
    case KIND_BUFFER: return child || guid_equal(iid, &IID_ID3D11Resource) || guid_equal(iid, &IID_ID3D11Buffer);
    case KIND_TEXTURE: return child || guid_equal(iid, &IID_ID3D11Resource) || guid_equal(iid, &IID_ID3D11Texture2D);
    case KIND_QUERY: return child || guid_equal(iid, &IID_ID3D11Asynchronous) || guid_equal(iid, &IID_ID3D11Query);
    case KIND_RTV: return view || guid_equal(iid, &IID_ID3D11RenderTargetView);
    case KIND_DSV: return view || guid_equal(iid, &IID_ID3D11DepthStencilView);
    case KIND_UAV: return view || guid_equal(iid, &IID_ID3D11UnorderedAccessView);
    case KIND_SWAP_CHAIN:
        return guid_equal(iid, &IID_IDXGIObject) || guid_equal(iid, &IID_IDXGIDeviceSubObject) ||
               guid_equal(iid, &IID_IDXGISwapChain);
    }
    return 0;
}

static void release_private(Header *header) {
    for (int slot = 0; slot < PRIVATE_SLOTS; slot++) {
        void *value = header->values[slot];
        header->values[slot] = NULL;
        if (value) Release(value);
    }
}

static void destroy(Header *header) {
    release_private(header);
    if (header->kind == KIND_DEVICE) {
        Context *context = ((Device *)header)->context;
        release_private(&context->header);
        free(context);
        atomic_fetch_sub(&live_objects, 1);
    }
    if (header->kind == KIND_BUFFER) free(((Buffer *)header)->bytes);
    if (header->kind == KIND_TEXTURE) {
        Texture *texture = (Texture *)header;
        for (uint32_t at = 0; at < texture->count; at++) free(texture->bytes[at]);
        free(texture->bytes);
        free(texture->pitch);
        free(texture->rows);
        free(texture->mapped);
    }
    if (header->kind == KIND_RTV || header->kind == KIND_DSV || header->kind == KIND_UAV)
        Release(((View *)header)->resource);
    if (header->kind == KIND_SWAP_CHAIN) {
        SwapChain *swap_chain = (SwapChain *)header;
        for (uint32_t at = 0; at < swap_chain->count; at++) Release(swap_chain->buffers[at]);
    }
    free(header);
    atomic_fetch_sub(&live_objects, 1);
}

static uint32_t object_add_ref(void *this) {
    Header *header = this;
    uint32_t count = atomic_fetch_add(&header->references, 1) + 1;
    /* A child the program holds holds its device. */
    if (count == 1 && header->kind != KIND_DEVICE) AddRef(header->device);
    return count;
}

static uint32_t object_release(void *this) {
    Header *header = this;
    uint32_t count = atomic_fetch_sub(&header->references, 1) - 1;
    if (count == 0) {
        Device *device = header->device;
        int kind = header->kind;
        /* The immediate context lives as long as its device. */
        if (kind != KIND_CONTEXT) destroy(header);
        if (kind != KIND_DEVICE) Release(device);
    }
    return count;
}

static HRESULT object_query_interface(void *this, const GUID *iid, void **out) {
    Header *header = this;
    if (!out) return E_POINTER;
    if (iid && header->kind == KIND_CONTEXT && guid_equal(iid, &IID_ID3D11Multithread)) {
        object_add_ref(this);
        *out = &((Context *)this)->multithread;
        return S_OK;
    }
    if (iid && header->kind == KIND_TEXTURE && surface_interface(iid)) {
        object_add_ref(this);
        *out = &((Texture *)this)->surface;
        return S_OK;
    }
    if (!iid || !answers(header->kind, iid)) {
        *out = NULL;
        return E_NOINTERFACE;
    }
    object_add_ref(this);
    *out = this;
    return S_OK;
}

static void child_get_device(void *this, void **out) {
    Device *device = ((Header *)this)->device;
    AddRef(device);
    *out = device;
}

static HRESULT child_set_private_data_interface(void *this, const GUID *key, void *value) {
    Header *header = this;
    if (!key) return E_INVALIDARG;
    if (value) AddRef(value);
    pthread_mutex_lock(&header->private_lock);
    int found = -1, empty = -1;
    for (int slot = 0; slot < PRIVATE_SLOTS; slot++) {
        if (header->values[slot] && guid_equal(&header->keys[slot], key)) found = slot;
        else if (!header->values[slot] && empty < 0) empty = slot;
    }
    void *replaced = NULL;
    if (found >= 0) {
        replaced = header->values[found];
        header->values[found] = NULL;
        empty = found;
    }
    HRESULT status = S_OK;
    void *refused = NULL;
    if (value && empty < 0) {
        status = E_OUTOFMEMORY;
        refused = value;
    } else if (value) {
        header->keys[empty] = *key;
        header->values[empty] = value;
    }
    pthread_mutex_unlock(&header->private_lock);
    /* Last, outside the lock, since the release may call this object again. */
    if (replaced) Release(replaced);
    if (refused) Release(refused);
    return status;
}

static void new_object(Header *header, Entry *table, int kind, Device *device) {
    pthread_mutex_init(&header->private_lock, NULL);
    header->table = table;
    atomic_init(&header->references, 0);
    header->kind = kind;
    header->device = device;
    atomic_fetch_add(&live_objects, 1);
}

static void resource_get_type(void *this, int32_t *dimension) {
    *dimension = ((Header *)this)->kind == KIND_TEXTURE ? D3D11_RESOURCE_DIMENSION_TEXTURE2D
                                                        : D3D11_RESOURCE_DIMENSION_BUFFER;
}

static void buffer_get_desc(void *this, BUFFER_DESC *desc) { *desc = ((Buffer *)this)->desc; }

static void texture_get_desc(void *this, TEXTURE2D_DESC *desc) { *desc = ((Texture *)this)->desc; }

static uint32_t query_data_size(const Query *query) {
    return query->desc.Query == D3D11_QUERY_EVENT ? sizeof(uint32_t) : sizeof(PIPELINE_STATISTICS);
}

static uint32_t asynchronous_get_data_size(void *this) { return query_data_size(this); }

static void query_get_desc(void *this, QUERY_DESC *desc) { *desc = ((Query *)this)->desc; }

/* The IDXGISurface of a texture: `this` is the address of the texture's `surface` field. */
static Texture *surface_texture(void *this) { return (Texture *)((char *)this - offsetof(Texture, surface)); }

static HRESULT surface_query_interface(void *this, const GUID *iid, void **out) {
    return object_query_interface(surface_texture(this), iid, out);
}

static uint32_t surface_add_ref(void *this) { return object_add_ref(surface_texture(this)); }

static uint32_t surface_release(void *this) { return object_release(surface_texture(this)); }

static HRESULT surface_get_desc(void *this, SURFACE_DESC *out) {
    Texture *texture = surface_texture(this);
    if (!out) return E_INVALIDARG;
    out->Width = texture->desc.Width;
    out->Height = texture->desc.Height;
    out->Format = texture->desc.Format;
    out->SampleDesc = texture->desc.SampleDesc;
    return S_OK;
}

/* ID3D11Multithread of the immediate context: `this` is the address of the context's `multithread` field. */
static Context *multithread_context(void *this) { return (Context *)((char *)this - offsetof(Context, multithread)); }

static HRESULT object_query_interface(void *this, const GUID *iid, void **out);
static uint32_t object_add_ref(void *this);
static uint32_t object_release(void *this);

static HRESULT multithread_query_interface(void *this, const GUID *iid, void **out) {
    return object_query_interface(multithread_context(this), iid, out);
}
static uint32_t multithread_add_ref(void *this) { return object_add_ref(multithread_context(this)); }
static uint32_t multithread_release(void *this) { return object_release(multithread_context(this)); }
static void multithread_enter(void *this) { pthread_mutex_lock(&multithread_context(this)->lock); }
static void multithread_leave(void *this) { pthread_mutex_unlock(&multithread_context(this)->lock); }
static int32_t multithread_set_protected(void *this, int32_t on) {
    int32_t before = atomic_exchange(&multithread_context(this)->protected_, on ? 1 : 0);
    if (on && !before) OBSERVE(observed.protected_contexts++);
    return before;
}
static int32_t multithread_get_protected(void *this) { return atomic_load(&multithread_context(this)->protected_); }
static Entry multithread_table[7] = {(Entry)multithread_query_interface, (Entry)multithread_add_ref,
                                     (Entry)multithread_release,         (Entry)multithread_enter,
                                     (Entry)multithread_leave,           (Entry)multithread_set_protected,
                                     (Entry)multithread_get_protected};

static Entry buffer_table[11];
static Entry texture_table[11];
static Entry surface_table[11];
static Entry view_table[9];
static Entry query_table[9];
static Entry swap_chain_table[18];

/* The fake's few formats: bytes per block, and the block's side (1, or 4 for BC1). */
static int format_block(uint32_t format, uint32_t *bytes, uint32_t *side) {
    *side = 1;
    switch (format) {
    case DXGI_FORMAT_R32G32B32A32_FLOAT: *bytes = 16; return 1;
    case DXGI_FORMAT_R8G8B8A8_UNORM:
    case DXGI_FORMAT_B8G8R8A8_UNORM:
    case DXGI_FORMAT_D24_UNORM_S8_UINT: *bytes = 4; return 1;
    case DXGI_FORMAT_BC1_UNORM: *bytes = 8; *side = 4; return 1;
    }
    return 0;
}

static HRESULT device_create_buffer(void *this, const BUFFER_DESC *desc, const SUBRESOURCE_DATA *initial, void **out) {
    if (!desc || desc->ByteWidth == 0) return E_INVALIDARG;
    if (desc->Usage == D3D11_USAGE_IMMUTABLE && !initial) return E_INVALIDARG;
    if (initial && !initial->pSysMem) return E_INVALIDARG;
    if (!out) return S_FALSE;
    Buffer *buffer = calloc(1, sizeof *buffer);
    buffer->desc = *desc;
    buffer->bytes = calloc(1, desc->ByteWidth);
    if (initial) memcpy(buffer->bytes, initial->pSysMem, desc->ByteWidth);
    new_object(&buffer->header, buffer_table, KIND_BUFFER, this);
    object_add_ref(buffer);
    *out = buffer;
    return S_OK;
}

static HRESULT device_create_texture_2d(void *this, const TEXTURE2D_DESC *desc, const SUBRESOURCE_DATA *initial,
                                        void **out) {
    uint32_t bytes = 0, side = 1;
    if (!desc || !desc->Width || !desc->Height || !desc->ArraySize || !format_block(desc->Format, &bytes, &side))
        return E_INVALIDARG;
    if (desc->Usage == D3D11_USAGE_IMMUTABLE && !initial) return E_INVALIDARG;
    if (!out) return S_FALSE;
    uint32_t mips = desc->MipLevels;
    if (!mips)
        for (uint32_t size = desc->Width > desc->Height ? desc->Width : desc->Height; size; size >>= 1) mips++;
    Texture *texture = calloc(1, sizeof *texture);
    texture->surface = surface_table;
    texture->desc = *desc;
    texture->desc.MipLevels = mips;
    texture->count = mips * desc->ArraySize;
    texture->pitch = calloc(texture->count, sizeof *texture->pitch);
    texture->rows = calloc(texture->count, sizeof *texture->rows);
    texture->bytes = calloc(texture->count, sizeof *texture->bytes);
    texture->mapped = calloc(texture->count, 1);
    for (uint32_t at = 0; at < texture->count; at++) {
        uint32_t mip = at % mips, width = desc->Width >> mip, height = desc->Height >> mip;
        width = width ? width : 1;
        height = height ? height : 1;
        uint32_t row_bytes = (width + side - 1) / side * bytes;
        texture->rows[at] = (height + side - 1) / side;
        texture->pitch[at] = (row_bytes + ROW_ALIGNMENT - 1) / ROW_ALIGNMENT * ROW_ALIGNMENT;
        texture->bytes[at] = calloc(texture->rows[at], texture->pitch[at]);
        for (uint32_t row = 0; initial && row < texture->rows[at]; row++)
            memcpy(texture->bytes[at] + row * texture->pitch[at],
                   (const uint8_t *)initial[at].pSysMem + row * initial[at].SysMemPitch, row_bytes);
    }
    new_object(&texture->header, texture_table, KIND_TEXTURE, this);
    object_add_ref(texture);
    *out = texture;
    return S_OK;
}

static HRESULT create_view(void *this, void *resource, int kind, void **out) {
    int resource_kind = resource ? ((Header *)resource)->kind : -1;
    if (resource_kind != KIND_BUFFER && resource_kind != KIND_TEXTURE) return E_INVALIDARG;
    if (!out) return S_FALSE;
    View *view = calloc(1, sizeof *view);
    view->resource = resource;
    AddRef(resource);
    new_object(&view->header, view_table, kind, this);
    object_add_ref(view);
    *out = view;
    return S_OK;
}

static HRESULT device_create_unordered_access_view(void *this, void *resource, const void *desc, void **out) {
    (void)desc;
    return create_view(this, resource, KIND_UAV, out);
}

static HRESULT device_create_render_target_view(void *this, void *resource, const void *desc, void **out) {
    (void)desc;
    return create_view(this, resource, KIND_RTV, out);
}

static HRESULT device_create_depth_stencil_view(void *this, void *resource, const void *desc, void **out) {
    (void)desc;
    return create_view(this, resource, KIND_DSV, out);
}

static HRESULT device_create_query(void *this, const QUERY_DESC *desc, void **out) {
    if (!desc || (desc->Query != D3D11_QUERY_EVENT && desc->Query != D3D11_QUERY_PIPELINE_STATISTICS))
        return E_INVALIDARG;
    if (!out) return S_FALSE;
    Query *query = calloc(1, sizeof *query);
    query->desc = *desc;
    new_object(&query->header, query_table, KIND_QUERY, this);
    object_add_ref(query);
    *out = query;
    return S_OK;
}

static void device_get_immediate_context(void *this, void **out) {
    Context *context = ((Device *)this)->context;
    object_add_ref(context);
    *out = context;
}

static void context_draw(void *this, uint32_t vertex_count, uint32_t start) {
    (void)start;
    enter(this);
    ((Context *)this)->vertices += vertex_count;
    leave(this);
}

static int mappable(int32_t usage, uint32_t access, int32_t type) {
    int reads = type == D3D11_MAP_READ || type == D3D11_MAP_READ_WRITE, writes = type != D3D11_MAP_READ;
    if (usage == D3D11_USAGE_DYNAMIC) return type == D3D11_MAP_WRITE_DISCARD || type == D3D11_MAP_WRITE_NO_OVERWRITE;
    if (usage == D3D11_USAGE_STAGING)
        return type >= D3D11_MAP_READ && type <= D3D11_MAP_READ_WRITE && (!reads || (access & D3D11_CPU_ACCESS_READ)) &&
               (!writes || (access & D3D11_CPU_ACCESS_WRITE));
    return 0;
}

static HRESULT context_map_locked(void *this, void *resource, uint32_t subresource, int32_t type, uint32_t flags,
                                  MAPPED_SUBRESOURCE *out) {
    (void)this;
    (void)flags;
    int kind = resource ? ((Header *)resource)->kind : -1;
    if (!out) return E_INVALIDARG;
    if (kind == KIND_TEXTURE) {
        Texture *texture = resource;
        if (subresource >= texture->count || texture->mapped[subresource] ||
            !mappable(texture->desc.Usage, texture->desc.CPUAccessFlags, type))
            return E_INVALIDARG;
        texture->mapped[subresource] = 1;
        OBSERVE(observed.mapped_now++);
        out->pData = texture->bytes[subresource];
        out->RowPitch = texture->pitch[subresource];
        out->DepthPitch = texture->pitch[subresource] * texture->rows[subresource];
        return S_OK;
    }
    Buffer *buffer = resource;
    if (kind != KIND_BUFFER || subresource != 0) return E_INVALIDARG;
    if (!mappable(buffer->desc.Usage, buffer->desc.CPUAccessFlags, type) || buffer->mapped) return E_INVALIDARG;
    buffer->mapped = 1;
    OBSERVE(observed.mapped_now++);
    out->pData = buffer->bytes;
    out->RowPitch = buffer->desc.ByteWidth;
    out->DepthPitch = buffer->desc.ByteWidth;
    return S_OK;
}

static HRESULT context_map(void *this, void *resource, uint32_t subresource, int32_t type, uint32_t flags,
                           MAPPED_SUBRESOURCE *out) {
    enter(this);
    HRESULT status = context_map_locked(this, resource, subresource, type, flags, out);
    leave(this);
    return status;
}

static void context_unmap(void *this, void *resource, uint32_t subresource) {
    enter(this);
    int kind = resource ? ((Header *)resource)->kind : -1;
    if (kind == KIND_TEXTURE && subresource < ((Texture *)resource)->count && ((Texture *)resource)->mapped[subresource]) {
        ((Texture *)resource)->mapped[subresource] = 0;
        OBSERVE(observed.mapped_now--);
    }
    if (kind == KIND_BUFFER && ((Buffer *)resource)->mapped) {
        ((Buffer *)resource)->mapped = 0;
        OBSERVE(observed.mapped_now--);
    }
    leave(this);
}

static void context_ia_set_vertex_buffers(void *this, uint32_t start, uint32_t count, void *const *buffers,
                                          const uint32_t *strides, const uint32_t *offsets) {
    Context *context = this;
    OBSERVE((observed.ia_calls++, observed.ia_start = start, observed.ia_count = count));
    if (start + count > VERTEX_SLOTS) return;
    enter(this);
    for (uint32_t slot = 0; slot < count; slot++) {
        void *now = buffers ? buffers[slot] : NULL, *before = context->bound[start + slot];
        if (atomic_load(&holds_bound) && now) AddRef(now);
        context->bound[start + slot] = now;
        context->strides[start + slot] = strides ? strides[slot] : 0;
        context->offsets[start + slot] = offsets ? offsets[slot] : 0;
        /* Inside the call, under the lock: a release that calls back re-enters on this thread. */
        if (atomic_load(&holds_bound) && before) Release(before);
    }
    leave(this);
}

static void context_output_merger(void *this, uint32_t target_count, void *const *targets, void *depth,
                                  uint32_t view_start, uint32_t view_count, void *const *views,
                                  const uint32_t *counts) {
    (void)counts;
    Context *context = this;
    enter(this);
    pthread_mutex_lock(&observed_lock);
    observed.om_calls++;
    if (target_count != D3D11_KEEP_RENDER_TARGETS_AND_DEPTH_STENCIL) {
        if (target_count > TARGET_SLOTS) goto done;
        memset(context->targets, 0, sizeof context->targets);
        for (uint32_t at = 0; at < target_count; at++) context->targets[at] = targets ? targets[at] : NULL;
        context->depth = depth;
    }
    if (view_count != D3D11_KEEP_UNORDERED_ACCESS_VIEWS) {
        if (view_start >= VIEW_SLOTS || view_count > VIEW_SLOTS - view_start) goto done;
        memset(context->views, 0, sizeof context->views);
        for (uint32_t at = 0; at < view_count; at++) context->views[view_start + at] = views ? views[at] : NULL;
    }
    observed.om_targets = observed.om_views = 0;
    observed.om_first_view = VIEW_SLOTS;
    for (int at = 0; at < TARGET_SLOTS; at++) observed.om_targets += context->targets[at] != NULL;
    for (int at = VIEW_SLOTS - 1; at >= 0; at--)
        if (context->views[at]) {
            observed.om_views++;
            observed.om_first_view = (uint32_t)at;
        }
    observed.om_depth = context->depth != NULL;
done:
    pthread_mutex_unlock(&observed_lock);
    leave(this);
}

static void context_begin(void *this, void *asynchronous) {
    Query *query = asynchronous;
    enter(this);
    if (query->desc.Query == D3D11_QUERY_PIPELINE_STATISTICS) {
        query->begun = 1;
        query->ended = 0;
        query->begun_at = ((Context *)this)->vertices;
    }
    leave(this);
}

static void context_end(void *this, void *asynchronous) {
    Query *query = asynchronous;
    enter(this);
    if (query->desc.Query != D3D11_QUERY_PIPELINE_STATISTICS || query->begun) {
        if (query->desc.Query == D3D11_QUERY_PIPELINE_STATISTICS)
            query->vertices = ((Context *)this)->vertices - query->begun_at;
        query->ended = 1;
    }
    leave(this);
}

static HRESULT context_get_data_locked(void *this, void *asynchronous, void *data, uint32_t size, uint32_t flags) {
    (void)this;
    (void)flags;
    Query *query = asynchronous;
    if (data && size != query_data_size(query)) return E_INVALIDARG;
    if (!query->ended) return S_FALSE;
    if (data && query->desc.Query == D3D11_QUERY_EVENT) {
        uint32_t reached = 1;
        memcpy(data, &reached, sizeof reached);
    } else if (data) {
        PIPELINE_STATISTICS statistics = {{0}};
        statistics.counts[0] = query->vertices;
        statistics.counts[1] = query->vertices / 3;
        statistics.counts[2] = query->vertices;
        memcpy(data, &statistics, sizeof statistics);
    }
    return S_OK;
}

static HRESULT context_get_data(void *this, void *asynchronous, void *data, uint32_t size, uint32_t flags) {
    enter(this);
    HRESULT status = context_get_data_locked(this, asynchronous, data, size, flags);
    leave(this);
    return status;
}

static HRESULT swap_chain_present(void *this, uint32_t interval, uint32_t flags) {
    (void)interval;
    (void)flags;
    ((SwapChain *)this)->presents++;
    return S_OK;
}

static HRESULT swap_chain_get_buffer(void *this, uint32_t index, const GUID *iid, void **out) {
    SwapChain *swap_chain = this;
    if (!out) return E_POINTER;
    *out = NULL;
    if (index >= swap_chain->count) return DXGI_ERROR_INVALID_CALL;
    return QueryInterface(swap_chain->buffers[index], iid, out);
}

#define UNKNOWN_ENTRIES [0] = (Entry)object_query_interface, [1] = (Entry)object_add_ref, [2] = (Entry)object_release
#define CHILD_ENTRIES UNKNOWN_ENTRIES, [3] = (Entry)child_get_device, [6] = (Entry)child_set_private_data_interface

static Entry device_table[41] = {UNKNOWN_ENTRIES,
                                 [3] = (Entry)device_create_buffer,
                                 [5] = (Entry)device_create_texture_2d,
                                 [8] = (Entry)device_create_unordered_access_view,
                                 [9] = (Entry)device_create_render_target_view,
                                 [10] = (Entry)device_create_depth_stencil_view,
                                 [24] = (Entry)device_create_query,
                                 [40] = (Entry)device_get_immediate_context};
static Entry context_table[35] = {CHILD_ENTRIES,
                                  [13] = (Entry)context_draw,
                                  [14] = (Entry)context_map,
                                  [15] = (Entry)context_unmap,
                                  [18] = (Entry)context_ia_set_vertex_buffers,
                                  [27] = (Entry)context_begin,
                                  [28] = (Entry)context_end,
                                  [29] = (Entry)context_get_data,
                                  [34] = (Entry)context_output_merger};
static Entry buffer_table[11] = {CHILD_ENTRIES, [7] = (Entry)resource_get_type, [10] = (Entry)buffer_get_desc};
static Entry texture_table[11] = {CHILD_ENTRIES, [7] = (Entry)resource_get_type, [10] = (Entry)texture_get_desc};
static Entry surface_table[11] = {[0] = (Entry)surface_query_interface, [1] = (Entry)surface_add_ref,
                                  [2] = (Entry)surface_release, [8] = (Entry)surface_get_desc};
static Entry view_table[9] = {CHILD_ENTRIES};
static Entry query_table[9] = {CHILD_ENTRIES, [7] = (Entry)asynchronous_get_data_size, [8] = (Entry)query_get_desc};
static Entry swap_chain_table[18] = {UNKNOWN_ENTRIES, [8] = (Entry)swap_chain_present, [9] = (Entry)swap_chain_get_buffer};

HRESULT D3D11CreateDevice(void *adapter, int32_t driver_type, void *software, uint32_t flags, const int32_t *levels,
                          uint32_t level_count, uint32_t sdk_version, void **device_out, int32_t *level_out,
                          void **context_out) {
    (void)adapter;
    (void)software;
    OBSERVE((observed.create_calls++, observed.create_flags = flags, observed.create_driver_type = driver_type));
    if (sdk_version != D3D11_SDK_VERSION) return E_INVALIDARG;
    int32_t level = D3D_FEATURE_LEVEL_11_0;
    if (levels && level_count) {
        level = 0;
        for (uint32_t at = 0; at < level_count && !level; at++)
            if (levels[at] >= D3D_FEATURE_LEVEL_9_1 && levels[at] <= D3D_FEATURE_LEVEL_11_1) level = levels[at];
        if (!level) return E_INVALIDARG;
    }
    if (level_out) *level_out = level;
    if (!device_out && !context_out) return S_FALSE;
    Device *device = calloc(1, sizeof *device);
    Context *context = calloc(1, sizeof *context);
    device->context = context;
    context->multithread = multithread_table;
    pthread_mutexattr_t recursive;
    pthread_mutexattr_init(&recursive);
    pthread_mutexattr_settype(&recursive, PTHREAD_MUTEX_RECURSIVE);
    pthread_mutex_init(&context->lock, &recursive);
    pthread_mutexattr_destroy(&recursive);
    new_object(&device->header, device_table, KIND_DEVICE, device);
    new_object(&context->header, context_table, KIND_CONTEXT, device);
    object_add_ref(device);
    if (context_out) {
        object_add_ref(context);
        *context_out = context;
    }
    if (device_out) *device_out = device;
    else object_release(device);
    return S_OK;
}

HRESULT D3D11CreateDeviceAndSwapChain(void *adapter, int32_t driver_type, void *software, uint32_t flags,
                                      const int32_t *levels, uint32_t level_count, uint32_t sdk_version,
                                      const SWAP_CHAIN_DESC *desc, void **swap_chain_out, void **device_out,
                                      int32_t *level_out, void **context_out) {
    if (swap_chain_out && (!desc || !desc->BufferCount || desc->BufferCount > BACK_BUFFERS)) return E_INVALIDARG;
    void *device = NULL, *context = NULL;
    HRESULT status = D3D11CreateDevice(adapter, driver_type, software, flags, levels, level_count, sdk_version,
                                       &device, level_out, &context);
    if (status != S_OK) return status;
    if (swap_chain_out) {
        SwapChain *swap_chain = calloc(1, sizeof *swap_chain);
        new_object(&swap_chain->header, swap_chain_table, KIND_SWAP_CHAIN, device);
        TEXTURE2D_DESC buffer = {desc->BufferDesc.Width, desc->BufferDesc.Height, 1, 1, desc->BufferDesc.Format,
                                 desc->SampleDesc, D3D11_USAGE_DEFAULT, D3D11_BIND_RENDER_TARGET, 0, 0};
        for (uint32_t at = 0; at < desc->BufferCount; at++) {
            status = device_create_texture_2d(device, &buffer, NULL, &swap_chain->buffers[at]);
            if (status != S_OK) break;
            swap_chain->count++;
        }
        object_add_ref(swap_chain);
        if (status != S_OK) {
            object_release(swap_chain);
            Release(context);
            Release(device);
            return status;
        }
        *swap_chain_out = swap_chain;
    }
    if (device_out) *device_out = device;
    else Release(device);
    if (context_out) *context_out = context;
    else Release(context);
    return S_OK;
}
