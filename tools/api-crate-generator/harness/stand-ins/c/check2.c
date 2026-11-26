/* check-2: games through raw COM calls only. Some are correct programs that a lying or hostile safe implementation
   behind the serving glue meets; others break Direct3D's rules the way real programs do. Each writes a short report
   to `out`; the glue's answer, or the memory checker, is what is measured. */
#include <pthread.h>
#include <stdatomic.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "d3d11_slice.h"

static HRESULT make_device(CreateDevice create, void **device, void **context) {
    int32_t levels[] = {D3D_FEATURE_LEVEL_11_0}, level = 0;
    return create(NULL, D3D_DRIVER_TYPE_HARDWARE, NULL, 0, levels, 1, D3D11_SDK_VERSION, device, &level, context);
}

static void release_all(void *a, void *b, void *c) {
    if (a) Release(a);
    if (b) Release(b);
    if (c) Release(c);
}

static const BUFFER_DESC SMALL = {16, D3D11_USAGE_DEFAULT, D3D11_BIND_VERTEX_BUFFER, 0, 0, 0};
static const BUFFER_DESC DYNAMIC = {64, D3D11_USAGE_DYNAMIC, D3D11_BIND_VERTEX_BUFFER, D3D11_CPU_ACCESS_WRITE, 0, 0};
static const GUID KEY = {0xc4ec2002, 1, 2, {9, 9, 9, 9, 9, 9, 9, 9}};

/* Correct: asks a buffer for its device twice. */
int check2_get_device_twice(CreateDevice create, char *out, size_t capacity) {
    void *device = NULL, *context = NULL, *buffer = NULL, *first = NULL, *second = NULL;
    if (make_device(create, &device, &context) != S_OK) return -1;
    CreateBuffer(device, &SMALL, NULL, &buffer);
    GetDevice(buffer, &first);
    GetDevice(buffer, &second);
    snprintf(out, capacity, "GetDevice twice: the same pointer %d", first == second);
    release_all(first, second, buffer);
    release_all(context, device, NULL);
    return 0;
}

/* Correct: holds a texture, draws once, then asks its own texture for its device. Returns 1 when GetDevice left its
   out-parameter unwritten. */
int check2_held_texture_after_draw(CreateDevice create, char *out, size_t capacity) {
    void *device = NULL, *context = NULL, *texture = NULL;
    if (make_device(create, &device, &context) != S_OK) return -1;
    TEXTURE2D_DESC desc = {4, 4, 1, 1, DXGI_FORMAT_R8G8B8A8_UNORM, {1, 0}, D3D11_USAGE_DEFAULT,
                           D3D11_BIND_SHADER_RESOURCE, 0, 0};
    HRESULT made = CreateTexture2D(device, &desc, NULL, &texture);
    Draw(context, 3, 0);
    void *const sentinel = (void *)(uintptr_t)0x5e5e5e50, *got = sentinel;
    GetDevice(texture, &got);
    snprintf(out, capacity, "CreateTexture2D 0x%08x; after one Draw, GetDevice on the game's own texture %s",
             (uint32_t)made, got == sentinel ? "left its out-parameter unwritten" : "wrote it");
    if (got && got != sentinel) Release(got);
    Release(context);
    Release(device);
    return got == sentinel;
}

/* Correct: asks a resource its type, and reads the description of that type, as programs do before a cast. With
   `exact`, the description has exactly its own size, for the memory checker; else it sits before 64 marked bytes.
   Returns how many bytes past the D3D11_BUFFER_DESC were written. */
int check2_get_type_then_get_desc(CreateDevice create, int exact, char *out, size_t capacity) {
    void *device = NULL, *context = NULL, *texture = NULL;
    if (make_device(create, &device, &context) != S_OK) return -1;
    TEXTURE2D_DESC desc = {4, 4, 1, 1, DXGI_FORMAT_R8G8B8A8_UNORM, {1, 0}, D3D11_USAGE_DEFAULT,
                           D3D11_BIND_SHADER_RESOURCE, 0, 0};
    CreateTexture2D(device, &desc, NULL, &texture);
    int32_t dimension = 0;
    GetType(texture, &dimension);
    int past = 0;
    if (dimension == D3D11_RESOURCE_DIMENSION_BUFFER) {
        if (exact) {
            BUFFER_DESC *buffer_desc = malloc(sizeof *buffer_desc);
            BufferGetDesc(texture, buffer_desc);
            free(buffer_desc);
        } else {
            uint8_t *block = malloc(sizeof(BUFFER_DESC) + 64);
            memset(block, 0x5a, sizeof(BUFFER_DESC) + 64);
            BufferGetDesc(texture, (BUFFER_DESC *)block);
            for (size_t at = sizeof(BUFFER_DESC); at < sizeof(BUFFER_DESC) + 64; at++)
                if (block[at] != 0x5a) past = (int)(at - sizeof(BUFFER_DESC)) + 1;
            free(block);
        }
    }
    snprintf(out, capacity, "GetType = %d; GetDesc as that type wrote %d bytes past the %zu-byte D3D11_BUFFER_DESC",
             dimension, past, sizeof(BUFFER_DESC));
    release_all(texture, context, device);
    return past;
}

/* Correct: makes an immutable vertex buffer from `static const` bytes, which live in read-only memory. */
static const uint8_t VERTICES[64] = {1, 2, 3, 4};

int check2_const_initial(CreateDevice create, char *out, size_t capacity) {
    void *device = NULL, *context = NULL, *buffer = NULL;
    if (make_device(create, &device, &context) != S_OK) return -1;
    BUFFER_DESC desc = {64, D3D11_USAGE_IMMUTABLE, D3D11_BIND_VERTEX_BUFFER, 0, 0, 0};
    SUBRESOURCE_DATA initial = {VERTICES, 0, 0};
    HRESULT made = CreateBuffer(device, &desc, &initial, &buffer);
    snprintf(out, capacity, "CreateBuffer from static const bytes = 0x%08x", (uint32_t)made);
    release_all(buffer, context, device);
    return made;
}

/* A program's idiom: fills a mapped texture by DepthPitch, which Microsoft documents as the data's physical size. */
int check2_fill_by_depth_pitch(CreateDevice create, char *out, size_t capacity) {
    void *device = NULL, *context = NULL, *texture = NULL;
    if (make_device(create, &device, &context) != S_OK) return -1;
    TEXTURE2D_DESC desc = {1, 4, 1, 1, DXGI_FORMAT_R8G8B8A8_UNORM, {1, 0}, D3D11_USAGE_DYNAMIC,
                           D3D11_BIND_SHADER_RESOURCE, D3D11_CPU_ACCESS_WRITE, 0};
    HRESULT made = CreateTexture2D(device, &desc, NULL, &texture);
    MAPPED_SUBRESOURCE mapped = {0};
    HRESULT status = made == S_OK ? Map(context, texture, 0, D3D11_MAP_WRITE_DISCARD, 0, &mapped) : made;
    if (status == S_OK && mapped.pData) memset(mapped.pData, 0xab, mapped.DepthPitch);
    snprintf(out, capacity, "Map = 0x%08x; RowPitch %u, DepthPitch %u; wrote DepthPitch bytes (rows at the pitch, "
             "the last one row long: %u)", (uint32_t)status, mapped.RowPitch, mapped.DepthPitch,
             3 * mapped.RowPitch + 4);
    if (status == S_OK) Unmap(context, texture, 0);
    release_all(texture, context, device);
    return status;
}

/* A program's mistake: a texture larger than Direct3D 11 allows (16384 a side), as a fuzzer or a bad file asks. */
int check2_huge_texture(CreateDevice create, char *out, size_t capacity) {
    void *device = NULL, *context = NULL, *texture = NULL;
    if (make_device(create, &device, &context) != S_OK) return -1;
    TEXTURE2D_DESC desc = {1u << 20, 1u << 30, 1, 1, DXGI_FORMAT_R32G32B32A32_FLOAT, {1, 0}, D3D11_USAGE_DEFAULT,
                           D3D11_BIND_SHADER_RESOURCE, 0, 0};
    HRESULT made = CreateTexture2D(device, &desc, NULL, &texture);
    snprintf(out, capacity, "CreateTexture2D %u x %u = 0x%08x", desc.Width, desc.Height, (uint32_t)made);
    release_all(texture, context, device);
    return made;
}

/* A game object attached as private data that meddles: its AddRef (which the glue calls to keep it) releases the
   game's only reference to the buffer it is being attached to, and its Release draws and asks the dead buffer for
   its device. */
typedef struct {
    Entry *table;
    atomic_uint references;
    void *buffer;
    void *context;
    atomic_uint draws;
} Meddler;

static HRESULT meddler_query_interface(void *this, const GUID *iid, void **out) {
    if (!guid_equal(iid, &IID_IUnknown)) {
        *out = NULL;
        return E_NOINTERFACE;
    }
    atomic_fetch_add(&((Meddler *)this)->references, 1);
    *out = this;
    return S_OK;
}

static uint32_t meddler_add_ref(void *this) {
    Meddler *meddler = this;
    uint32_t count = atomic_fetch_add(&meddler->references, 1) + 1;
    void *buffer = meddler->buffer;
    if (buffer) {
        meddler->buffer = NULL;
        Release(buffer);
    }
    return count;
}

static uint32_t meddler_release(void *this) {
    Meddler *meddler = this;
    uint32_t count = atomic_fetch_sub(&meddler->references, 1) - 1;
    if (meddler->context) {
        Draw(meddler->context, 3, 0);
        atomic_fetch_add(&meddler->draws, 1);
    }
    return count;
}

static Entry meddler_table[3] = {(Entry)meddler_query_interface, (Entry)meddler_add_ref, (Entry)meddler_release};

int check2_callbacks(CreateDevice create, char *out, size_t capacity) {
    void *device = NULL, *context = NULL, *buffer = NULL;
    if (make_device(create, &device, &context) != S_OK) return -1;
    CreateBuffer(device, &SMALL, NULL, &buffer);
    Meddler meddler = {meddler_table, 1, buffer, context, 0};
    HRESULT attached = SetPrivateDataInterface(buffer, &KEY, &meddler);
    void *const sentinel = (void *)(uintptr_t)0x5e5e5e50, *got = sentinel;
    GetDevice(buffer, &got); /* the buffer is gone: the program's bug */
    meddler.context = NULL;
    snprintf(out, capacity, "attached 0x%08x; meddler references %u, draws from its Release %u; GetDevice on the "
             "dead buffer %s", (uint32_t)attached, atomic_load(&meddler.references), atomic_load(&meddler.draws),
             got == sentinel ? "wrote nothing" : "wrote a device");
    if (got && got != sentinel) Release(got);
    release_all(context, device, NULL);
    return 0;
}

/* A program's bug: maps a buffer, releases its last reference without Unmap and keeps writing; then maps a second
   buffer. Returns how many of the second mapping's bytes the stale writes changed. */
int check2_write_after_release(CreateDevice create, char *out, size_t capacity) {
    void *device = NULL, *context = NULL, *first = NULL, *second = NULL;
    if (make_device(create, &device, &context) != S_OK) return -1;
    CreateBuffer(device, &DYNAMIC, NULL, &first);
    MAPPED_SUBRESOURCE stale = {0}, fresh = {0};
    HRESULT mapped = Map(context, first, 0, D3D11_MAP_WRITE_DISCARD, 0, &stale);
    Release(first);
    if (mapped == S_OK) memset(stale.pData, 0xee, 64);
    CreateBuffer(device, &DYNAMIC, NULL, &second);
    HRESULT again = Map(context, second, 0, D3D11_MAP_WRITE_DISCARD, 0, &fresh);
    int changed = 0;
    if (again == S_OK) {
        memset(fresh.pData, 0x11, 64);
        if (mapped == S_OK) memset(stale.pData, 0xee, 64);
        for (int at = 0; at < 64; at++) changed += ((uint8_t *)fresh.pData)[at] != 0x11;
        Unmap(context, second, 0);
    }
    snprintf(out, capacity, "Map 0x%08x then Release with no Unmap; second Map 0x%08x %s the first's address; %d "
             "of its 64 bytes changed by stale writes", (uint32_t)mapped, (uint32_t)again,
             fresh.pData == stale.pData ? "reuses" : "does not reuse", changed);
    release_all(second, context, device);
    return changed;
}

/* A program's bug: threads map, write and unmap one buffer at once, and another unmaps at random, twice, and with
   a null resource and a wrong subresource. */
typedef struct {
    void *context, *buffer;
    uint32_t iterations;
    int unmapper;
    atomic_uint *maps;
} Racer;

static void *race(void *argument) {
    Racer *racer = argument;
    for (uint32_t at = 0; at < racer->iterations; at++) {
        if (racer->unmapper) {
            Unmap(racer->context, racer->buffer, 0);
            Unmap(racer->context, racer->buffer, 0);
            Unmap(racer->context, NULL, 0);
            Unmap(racer->context, racer->buffer, 7);
            continue;
        }
        MAPPED_SUBRESOURCE mapped = {0};
        if (Map(racer->context, racer->buffer, 0, D3D11_MAP_WRITE_DISCARD, 0, &mapped) == S_OK && mapped.pData) {
            atomic_fetch_add(racer->maps, 1);
            memset(mapped.pData, (int)at, 64);
            Unmap(racer->context, racer->buffer, 0);
        }
    }
    return NULL;
}

int check2_map_race(CreateDevice create, uint32_t threads, uint32_t iterations, char *out, size_t capacity) {
    void *device = NULL, *context = NULL, *buffer = NULL;
    if (make_device(create, &device, &context) != S_OK) return -1;
    CreateBuffer(device, &DYNAMIC, NULL, &buffer);
    atomic_uint maps = 0;
    pthread_t thread[16];
    Racer racer[16];
    if (threads > 16) threads = 16;
    for (uint32_t at = 0; at < threads; at++) {
        racer[at] = (Racer){context, buffer, iterations, at == 0, &maps};
        pthread_create(&thread[at], NULL, race, &racer[at]);
    }
    for (uint32_t at = 0; at < threads; at++) pthread_join(thread[at], NULL);
    snprintf(out, capacity, "%u threads x %u iterations: %u maps succeeded; returned", threads, iterations,
             atomic_load(&maps));
    release_all(buffer, context, device);
    return 0;
}
