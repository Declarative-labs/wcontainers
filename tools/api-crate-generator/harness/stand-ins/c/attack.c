/* The independent check's attacks, now tests that must pass, and three more: what careless or hostile programs do,
   through raw COM calls only. Each returns a small report in `out` and never crashes on purpose; the glue's answer is
   what is measured. Adapted from the check's attack.c. */
#include <pthread.h>
#include <stdatomic.h>
#include <stdio.h>
#include <stdlib.h>
#include <time.h>

#include "d3d11_slice.h"

static HRESULT make_device(CreateDevice create, void **device, void **context) {
    int32_t levels[] = {D3D_FEATURE_LEVEL_11_0}, level = 0;
    return create(NULL, D3D_DRIVER_TYPE_HARDWARE, NULL, 0, levels, 1, D3D11_SDK_VERSION, device, &level, context);
}

static const BUFFER_DESC SMALL = {16, D3D11_USAGE_DEFAULT, D3D11_BIND_VERTEX_BUFFER, 0, 0, 0};

/* A stale pointer after the last release, once another object of the same kind is made. Returns 1 when the stale
   pointer is the newer object's. */
int attack_stale_reuse(CreateDevice create, char *out, size_t capacity) {
    void *device = NULL, *context = NULL, *first = NULL, *second = NULL;
    if (make_device(create, &device, &context) != S_OK) return -1;
    BUFFER_DESC large = {32, D3D11_USAGE_DEFAULT, D3D11_BIND_VERTEX_BUFFER, 0, 0, 0};
    CreateBuffer(device, &SMALL, NULL, &first);
    void *stale = first;
    uint32_t left = Release(first);
    CreateBuffer(device, &large, NULL, &second);
    BUFFER_DESC seen = {0xdead, 0, 0, 0, 0, 0};
    BufferGetDesc(stale, &seen);
    uint32_t stale_release = Release(stale);
    BUFFER_DESC after = {0xdead, 0, 0, 0, 0, 0};
    BufferGetDesc(second, &after);
    snprintf(out, capacity,
             "released to %u; second == stale: %d; GetDesc(stale).ByteWidth = %u; Release(stale) = %u; "
             "GetDesc(second).ByteWidth after = %u (0 = no object)",
             left, second == stale, seen.ByteWidth, stale_release, after.ByteWidth);
    Release(second);
    Release(context);
    Release(device);
    return second == stale;
}

/* A stale pointer after `later` more objects of its kind were made and released. Returns 1 when it found one. */
int attack_stale_bound(CreateDevice create, uint32_t later, char *out, size_t capacity) {
    void *device = NULL, *context = NULL, *first = NULL;
    if (make_device(create, &device, &context) != S_OK) return -1;
    CreateBuffer(device, &SMALL, NULL, &first);
    void *stale = first, *reused = NULL;
    Release(first);
    for (uint32_t at = 0; at < later; at++) {
        void *buffer = NULL;
        CreateBuffer(device, &SMALL, NULL, &buffer);
        if (buffer == stale) reused = buffer;
        if (buffer) Release(buffer);
    }
    BUFFER_DESC seen = {0xdead, 0, 0, 0, 0, 0};
    BufferGetDesc(stale, &seen);
    uint32_t added = AddRef(stale);
    snprintf(out, capacity, "after %u later buffers: stale pointer reused %s; GetDesc(stale) = %#x; AddRef(stale) = %u",
             later, reused ? "yes" : "no", seen.ByteWidth, added);
    Release(context);
    Release(device);
    /* Found nothing: GetDesc writes zeros (an out-parameter is written on every path) and AddRef answers 0. */
    return seen.ByteWidth != 0 || added != 0;
}

/* Many live objects at once, then every one released, then an object of another kind. Returns how many times
   CreateBuffer answered S_OK with a null buffer. */
int attack_table_full(CreateDevice create, uint32_t count, uint32_t *made, uint32_t *refused, uint32_t *query_made,
                      char *out, size_t capacity) {
    void *device = NULL, *context = NULL;
    if (make_device(create, &device, &context) != S_OK) return -1;
    void **buffers = calloc(count, sizeof *buffers);
    uint32_t ok_but_null = 0, first_refused = 0;
    HRESULT last_refusal = S_OK;
    *made = *refused = 0;
    for (uint32_t at = 0; at < count; at++) {
        buffers[at] = (void *)1;
        HRESULT status = CreateBuffer(device, &SMALL, NULL, &buffers[at]);
        if (status == S_OK && buffers[at]) (*made)++;
        if (status == S_OK && !buffers[at]) ok_but_null++;
        if (status < 0) {
            if ((*refused)++ == 0) first_refused = at;
            last_refusal = status;
            if (buffers[at]) ok_but_null++; /* a failure must leave null */
            buffers[at] = NULL;
        }
    }
    for (uint32_t at = 0; at < count; at++)
        if (buffers[at]) Release(buffers[at]);
    free(buffers);
    QUERY_DESC event = {D3D11_QUERY_EVENT, 0};
    void *query = (void *)1;
    HRESULT query_status = CreateQuery(device, &event, &query);
    *query_made = query_status == S_OK && query;
    snprintf(out, capacity,
             "made %u of %u buffers; S_OK or a failure with a non-null buffer %u times; refused %u times, first at "
             "%u, with 0x%08x; after releasing all: CreateQuery = 0x%08x, query %s",
             *made, count, ok_but_null, *refused, first_refused, (uint32_t)last_refusal, (uint32_t)query_status,
             query ? "non-null" : "NULL");
    if (query_status == S_OK && query) Release(query);
    Release(context);
    Release(device);
    return (int)ok_but_null;
}

/* An object the game implements that holds the game's only reference to the buffer it is attached to; clearing the
   attachment releases the buffer: inside the call on that same buffer, or from another thread while it waits. */
typedef struct {
    Entry *table;
    atomic_uint references;
    void *owned;
    int other_thread;
    atomic_int signal; /* 1: release now, 2: released */
} Owner;

static HRESULT owner_query_interface(void *this, const GUID *iid, void **out) {
    if (!guid_equal(iid, &IID_IUnknown)) {
        *out = NULL;
        return E_NOINTERFACE;
    }
    atomic_fetch_add(&((Owner *)this)->references, 1);
    *out = this;
    return S_OK;
}

static uint32_t owner_add_ref(void *this) { return atomic_fetch_add(&((Owner *)this)->references, 1) + 1; }

/* Waits up to two seconds for the signal to reach `value`. */
static int wait_for(atomic_int *signal, int value) {
    struct timespec pause = {0, 1000000};
    for (int tries = 0; tries < 2000; tries++) {
        if (atomic_load(signal) >= value) return 1;
        nanosleep(&pause, NULL);
    }
    return 0;
}

static uint32_t owner_release(void *this) {
    Owner *owner = this;
    uint32_t count = atomic_fetch_sub(&owner->references, 1) - 1;
    void *owned = owner->owned;
    if (count == 1 && owned) {
        owner->owned = NULL;
        if (owner->other_thread) {
            atomic_store(&owner->signal, 1);
            wait_for(&owner->signal, 2);
        } else {
            Release(owned);
        }
    }
    return count;
}

static Entry owner_table[3] = {(Entry)owner_query_interface, (Entry)owner_add_ref, (Entry)owner_release};

typedef struct {
    Owner *owner;
    void *buffer;
    uint32_t released_to;
} Releaser;

static void *release_when_signaled(void *argument) {
    Releaser *releaser = argument;
    if (wait_for(&releaser->owner->signal, 1)) releaser->released_to = Release(releaser->buffer);
    atomic_store(&releaser->owner->signal, 2);
    return NULL;
}

static int release_inside(CreateDevice create, int other_thread, char *out, size_t capacity) {
    void *device = NULL, *context = NULL, *buffer = NULL;
    if (make_device(create, &device, &context) != S_OK) return -1;
    CreateBuffer(device, &SMALL, NULL, &buffer);
    static const GUID key = {0x5eed0009, 1, 2, {3, 3, 3, 3, 3, 3, 3, 3}};
    Owner owner = {owner_table, 1, NULL, other_thread, 0};
    HRESULT attached = SetPrivateDataInterface(buffer, &key, &owner);
    owner.owned = buffer; /* the game's only reference now lives in the owner */
    Releaser releaser = {&owner, buffer, 0xdead};
    pthread_t thread;
    if (other_thread) pthread_create(&thread, NULL, release_when_signaled, &releaser);
    HRESULT cleared = SetPrivateDataInterface(buffer, &key, NULL);
    if (other_thread) pthread_join(thread, NULL);
    snprintf(out, capacity,
             "attached 0x%08x, cleared 0x%08x, owner references %u, owner still owns %s, other thread released to %u",
             (uint32_t)attached, (uint32_t)cleared, atomic_load(&owner.references), owner.owned ? "yes" : "no",
             releaser.released_to);
    Release(context);
    Release(device);
    return 0;
}

int attack_release_inside_call(CreateDevice create, char *out, size_t capacity) {
    return release_inside(create, 0, out, capacity);
}

int attack_release_from_other_thread(CreateDevice create, char *out, size_t capacity) {
    return release_inside(create, 1, out, capacity);
}

/* A game that writes the whole buffer it mapped, as Direct3D's documentation allows. Returns Map's status. */
int attack_write_mapped(CreateDevice create, uint32_t width, char *out, size_t capacity) {
    void *device = NULL, *context = NULL, *buffer = NULL;
    if (make_device(create, &device, &context) != S_OK) return -1;
    BUFFER_DESC dynamic = {width, D3D11_USAGE_DYNAMIC, D3D11_BIND_VERTEX_BUFFER, D3D11_CPU_ACCESS_WRITE, 0, 0};
    CreateBuffer(device, &dynamic, NULL, &buffer);
    MAPPED_SUBRESOURCE mapped = {0};
    HRESULT status = Map(context, buffer, 0, D3D11_MAP_WRITE_DISCARD, 0, &mapped);
    if (status == S_OK && mapped.pData) memset(mapped.pData, 0xab, width);
    snprintf(out, capacity, "Map = 0x%08x, wrote %u bytes at %p", (uint32_t)status, status == S_OK ? width : 0,
             mapped.pData);
    if (status == S_OK) Unmap(context, buffer, 0);
    Release(buffer);
    Release(context);
    Release(device);
    return status;
}

/* The same for a texture: every row at the pitch Map gives, the last one row long. Returns Map's status. */
int attack_write_mapped_texture(CreateDevice create, uint32_t side, char *out, size_t capacity) {
    void *device = NULL, *context = NULL, *texture = NULL;
    if (make_device(create, &device, &context) != S_OK) return -1;
    TEXTURE2D_DESC desc = {side, side, 1, 1, DXGI_FORMAT_R8G8B8A8_UNORM, {1, 0}, D3D11_USAGE_DYNAMIC,
                           D3D11_BIND_SHADER_RESOURCE, D3D11_CPU_ACCESS_WRITE, 0};
    HRESULT created = CreateTexture2D(device, &desc, NULL, &texture);
    MAPPED_SUBRESOURCE mapped = {0};
    HRESULT status = created == S_OK ? Map(context, texture, 0, D3D11_MAP_WRITE_DISCARD, 0, &mapped) : created;
    if (status == S_OK && mapped.pData)
        for (uint32_t row = 0; row < side; row++) memset((uint8_t *)mapped.pData + row * mapped.RowPitch, 0xab, side * 4);
    snprintf(out, capacity, "CreateTexture2D = 0x%08x, Map = 0x%08x, RowPitch %u, at %p", (uint32_t)created,
             (uint32_t)status, mapped.RowPitch, mapped.pData);
    if (status == S_OK) Unmap(context, texture, 0);
    if (texture) Release(texture);
    Release(context);
    Release(device);
    return status;
}

/* Counts the output merger may not trust: too many render targets, a start slot past the last, and the keep values
   with garbage arrays, which must not be read. */
int attack_output_merger(CreateDevice create, char *out, size_t capacity) {
    void *device = NULL, *context = NULL;
    if (make_device(create, &device, &context) != S_OK) return -1;
    void *const *garbage = (void *const *)(uintptr_t)0x10;
    OMSetRenderTargetsAndUnorderedAccessViews(context, 9, garbage, NULL, 0, 0, NULL, NULL);
    OMSetRenderTargetsAndUnorderedAccessViews(context, 0, NULL, NULL, 64, 1, garbage, (const uint32_t *)garbage);
    OMSetRenderTargetsAndUnorderedAccessViews(context, 0, NULL, NULL, 60, 5, garbage, (const uint32_t *)garbage);
    OMSetRenderTargetsAndUnorderedAccessViews(context, D3D11_KEEP_RENDER_TARGETS_AND_DEPTH_STENCIL, garbage,
                                              (void *)garbage, 0, D3D11_KEEP_UNORDERED_ACCESS_VIEWS, garbage,
                                              (const uint32_t *)garbage);
    Draw(context, 3, 0);
    snprintf(out, capacity, "four calls out of range or kept, with garbage arrays: returned");
    Release(context);
    Release(device);
    return 0;
}
