/* Written by tools/api-crate-generator from Khronos's gl.xml and egl.xml, pinned in src/khronos/write.rs, and its template
 * templates/khronos/fake-egl.c. Do not edit. */
/* A stand-in EGL and OpenGL ES library for fuzzing the glue: it reads every byte and writes every value the registry
 * lets a command reach (the most a call may), so that a glue passing less memory than the call reaches is caught (by
 * AddressSanitizer's own check of each region, asked through its interface when the process has it, or by the touch
 * of each byte); and with FAKE_LIES set it lies wherever the registry leaves the answer to the library: random return
 * values and handles, null texts and mappings, more configurations written than there was room for, an extension
 * count far past any real one. It keeps the state the glue asks about (bindings, pixel store values, vertex
 * attributes) truthfully: the glue's marks rely on those answers.
 *
 * With FAKE_LOG set it writes each call's arguments to the file it names, one line a call (the direct crate's
 * tests/arguments.rs reads them): the command, then each argument, a number as C has it, a float to 9 digits, an offset
 * into a bound buffer as its number, a handle as `h` and its place among the library's, and what each pointer points at
 * (the values a command reads as `[a,b]`, a float as its bits, bytes as `bytes <count> <FNV-1a hash>`, a text as
 * `'text'`, an out pointer as `p`, null as `null`). Each line is followed by what the call gave back, each on a line of
 * its own: `  wrote <parameter> <values>` for every value it wrote (written as a pattern of its own, some bytes zero)
 * and `  returned <value>` (a value of its own: a number with its top bit set, a handle, the text). While the file
 * FAKE_LOG names with `.refuse` after it exists, each EGL command answering EGL_TRUE or EGL_FALSE refuses.
 *
 * Built as libEGL.so.1; the OpenGL ES commands are reached through eglGetProcAddress (the part written after this
 * template holds the commands no state is kept for and the table of every name). */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#define NONE_KEY 0x3038
#define ATTRIBUTES 16
#define MOST_CONFIGS 8

struct attribute {
    int enabled;
    uint32_t buffer;
    const void *pointer;
};

/* The state the glue asks about, truthfully kept. */
static struct {
    uint32_t array_buffer, element_buffer, unpack_buffer, pack_buffer, next_name;
    int32_t store[2][6]; /* [unpack, pack][alignment, row length, skip rows, skip pixels, image height, skip images] */
    struct attribute attributes[ATTRIBUTES];
    void *current;
    int lies, ready, logging;
    uint64_t patterned, returned, handles;
    uint64_t random;
    void *mapped;
    unsigned long reads, writes;
} fake;

static int (*region_poisoned)(const volatile void *, size_t);

static void fake_start(void) {
    if (fake.ready) return;
    fake.ready = 1;
    fake.lies = getenv("FAKE_LIES") != NULL;
    fake.logging = getenv("FAKE_LOG") != NULL;
    const char *seed = getenv("FAKE_SEED");
    fake.random = seed ? strtoull(seed, NULL, 10) | 1 : 0x9E3779B97F4A7C15ull;
    fake.store[0][0] = fake.store[1][0] = 4;
    fake.next_name = 1;
    /* AddressSanitizer's interface, when the process was built with it. */
    region_poisoned = (int (*)(const volatile void *, size_t))dlsym(RTLD_DEFAULT, "__asan_region_is_poisoned");
    if (getenv("FAKE_SAY")) fprintf(stderr, "FAKE: lies %d, AddressSanitizer's region check %s\n", fake.lies, region_poisoned ? "found" : "absent");
}

/* One call's line in the log FAKE_LOG names, when it names one. */
static void fake_log(const char *format, ...) {
    static FILE *file;
    static int opened;
    fake_start();
    if (!opened) {
        opened = 1;
        const char *path = getenv("FAKE_LOG");
        if (path) file = fopen(path, "w");
    }
    if (!file) return;
    va_list arguments;
    va_start(arguments, format);
    vfprintf(file, format, arguments);
    va_end(arguments);
    fputc('\n', file);
    fflush(file);
}

/* An address as the log writes it. */
#define FAKE_ADDRESS(pointer) ((pointer) ? "p" : "null")

/* The texts the log's parts are written into: each part's own, reused after PARTS more. */
#define PARTS 16
#define PART_BYTES 65536
static char *fake_part(void) {
    static char parts[PARTS][PART_BYTES];
    static unsigned next;
    char *part = parts[next++ % PARTS];
    part[0] = 0;
    return part;
}

/* `count` values of a kind at `pointer` as the log writes them: `i` int32_t, `u` uint32_t, `f` a float's bits, `I`
 * int64_t, `U` uint64_t, `b` uint8_t; nothing is read unless the run logs. */
static const char *fake_values(const void *pointer, size_t count, char kind) {
    if (!fake.logging) return "";
    if (pointer == NULL) return "null";
    char *part = fake_part();
    size_t used = 0;
    used += (size_t)snprintf(part + used, PART_BYTES - used, "[");
    for (size_t at = 0; at < count && used + 32 < PART_BYTES; at++) {
        const char *comma = at ? "," : "";
        switch (kind) {
        case 'i': { int32_t v; memcpy(&v, (const uint8_t *)pointer + at * 4, 4); used += (size_t)snprintf(part + used, PART_BYTES - used, "%s%lld", comma, (long long)v); break; }
        case 'u': { uint32_t v; memcpy(&v, (const uint8_t *)pointer + at * 4, 4); used += (size_t)snprintf(part + used, PART_BYTES - used, "%s%llu", comma, (unsigned long long)v); break; }
        case 'f': { uint32_t v; memcpy(&v, (const uint8_t *)pointer + at * 4, 4); used += (size_t)snprintf(part + used, PART_BYTES - used, "%s0x%x", comma, (unsigned)v); break; }
        case 'I': { int64_t v; memcpy(&v, (const uint8_t *)pointer + at * 8, 8); used += (size_t)snprintf(part + used, PART_BYTES - used, "%s%lld", comma, (long long)v); break; }
        case 'U': { uint64_t v; memcpy(&v, (const uint8_t *)pointer + at * 8, 8); used += (size_t)snprintf(part + used, PART_BYTES - used, "%s%llu", comma, (unsigned long long)v); break; }
        default: used += (size_t)snprintf(part + used, PART_BYTES - used, "%s%u", comma, (unsigned)((const uint8_t *)pointer)[at]); break;
        }
    }
    snprintf(part + used, PART_BYTES - used, "]");
    return part;
}

/* `count` bytes at `pointer` as the log writes them: their count and FNV-1a hash. */
static const char *fake_bytes(const void *pointer, size_t count) {
    if (!fake.logging) return "";
    if (pointer == NULL) return "null";
    uint64_t hash = 0xcbf29ce484222325ull;
    for (size_t at = 0; at < count; at++) hash = (hash ^ ((const uint8_t *)pointer)[at]) * 0x100000001b3ull;
    char *part = fake_part();
    snprintf(part, PART_BYTES, "bytes %zu %016llx", count, (unsigned long long)hash);
    return part;
}

/* A zero-ended text as the log writes it. */
static const char *fake_text(const void *text) {
    if (!fake.logging) return "";
    if (text == NULL) return "null";
    char *part = fake_part();
    snprintf(part, PART_BYTES, "'%s'", (const char *)text);
    return part;
}

/* `count` texts, each `lengths[k]` bytes long (zero-ended where lengths is null or the length is negative). */
static const char *fake_texts(const uint8_t *const *texts, int32_t count, const int32_t *lengths) {
    if (!fake.logging) return "";
    if (texts == NULL) return "null";
    char *part = fake_part();
    size_t used = (size_t)snprintf(part, PART_BYTES, "[");
    for (int32_t at = 0; at < count && used + 64 < PART_BYTES; at++) {
        int length = lengths && lengths[at] >= 0 ? (int)lengths[at] : (int)strlen((const char *)texts[at]);
        used += (size_t)snprintf(part + used, PART_BYTES - used, "%s'%.*s'", at ? "," : "", length, (const char *)texts[at]);
    }
    snprintf(part + used, PART_BYTES - used, "]");
    return part;
}

static uint8_t tokens[4096];

/* A handle as the log writes it: `h` and its place among the library's. */
static const char *fake_handle_name(const void *handle) {
    if (!fake.logging) return "";
    if (handle == NULL) return "null";
    char *part = fake_part();
    const uint8_t *at = handle;
    if (at >= tokens && at < tokens + sizeof tokens) snprintf(part, PART_BYTES, "h%td", at - tokens);
    else snprintf(part, PART_BYTES, "p");
    return part;
}

/* `count` handles at `pointer`, as the log writes them. */
static const char *fake_handles(void *const *pointer, size_t count) {
    if (!fake.logging) return "";
    if (pointer == NULL) return "null";
    char *part = fake_part();
    size_t used = (size_t)snprintf(part, PART_BYTES, "[");
    for (size_t at = 0; at < count && used + 32 < PART_BYTES; at++) used += (size_t)snprintf(part + used, PART_BYTES - used, "%s%s", at ? "," : "", fake_handle_name(pointer[at]));
    snprintf(part + used, PART_BYTES - used, "]");
    return part;
}

/* A value a command returns in a logged run: of its own, its top bit set (a signed one negative). */
uint64_t fake_returned(void) {
    fake.returned++;
    return 0xFFFFFFFF80000000ull | ((fake.returned * 0x9E3779B1ull) & 0x7FFFFFF0ull) | (fake.returned & 0xF);
}

/* Whether an EGL command refuses: while FAKE_LOG's file with `.refuse` after it exists. */
static int fake_refusing(void) {
    if (!fake.logging) return 0;
    char path[4096];
    snprintf(path, sizeof path, "%s.refuse", getenv("FAKE_LOG"));
    return access(path, F_OK) == 0;
}

/* What the stand-in reached, said at the end when FAKE_SAY is set: a run that reached nothing proves nothing. */
__attribute__((destructor)) static void fake_end(void) {
    if (fake.ready && getenv("FAKE_SAY")) fprintf(stderr, "FAKE: reads %lu, writes %lu\n", fake.reads, fake.writes);
}

uint64_t fake_next(void) {
    fake.random ^= fake.random << 13;
    fake.random ^= fake.random >> 7;
    fake.random ^= fake.random << 17;
    return fake.random;
}

static void fake_fail(const char *what, const char *command, const void *pointer, size_t bytes) {
    fprintf(stderr, "FAKE: %s by %s at %p, %zu bytes\n", what, command, pointer, bytes);
    abort();
}

/* Reads every byte of a region the command reaches. */
void fake_read(const char *command, const void *pointer, size_t bytes) {
    if (bytes == 0) return;
    if (pointer == NULL) fake_fail("a read of null", command, pointer, bytes);
    if (region_poisoned && region_poisoned(pointer, bytes)) fake_fail("a read past what was given", command, pointer, bytes);
    volatile uint8_t sum = 0;
    for (size_t at = 0; at < bytes; at++) sum ^= ((const volatile uint8_t *)pointer)[at];
    (void)sum;
    fake.reads++;
}

/* Writes every byte of a region the command reaches. */
void fake_write(const char *command, void *pointer, size_t bytes) {
    if (bytes == 0) return;
    if (pointer == NULL) fake_fail("a write to null", command, pointer, bytes);
    if (region_poisoned && region_poisoned(pointer, bytes)) fake_fail("a write past what was given", command, pointer, bytes);
    for (size_t at = 0; at < bytes; at++) {
        /* Logged, a pattern of the log's own (every third byte zero, the others never); else random. */
        uint64_t n = fake.patterned++;
        ((volatile uint8_t *)pointer)[at] = fake.logging ? (n % 3 == 2 ? 0 : (uint8_t)((n * 0x9D + 0x41) | 1)) : (uint8_t)fake_next();
    }
    fake.writes++;
}

/* Reads a zero-ended text. */
void fake_read_text(const char *command, const void *text) {
    if (text == NULL) fake_fail("a read of a null text", command, text, 0);
    fake_read(command, text, strlen((const char *)text) + 1);
}

/* A value the registry leaves to the library: random when lying, else 0. */
uint64_t fake_value(void) {
    fake_start();
    return fake.lies ? fake_next() : 0;
}

/* The bytes of one pixel of a format and type (OpenGL ES 3.2 tables 8.2, 8.3), or 0 when not one of them. */
static size_t pixel_bytes(uint32_t format, uint32_t type) {
    switch (type) {
    case 0x8363: case 0x8033: case 0x8034: return 2;                 /* 5_6_5, 4_4_4_4, 5_5_5_1 */
    case 0x8368: case 0x8C3B: case 0x8C3E: case 0x84FA: return 4;     /* 2_10_10_10_REV, 10F_11F_11F_REV, 5_9_9_9_REV, 24_8 */
    case 0x8DAD: return 8;                                            /* FLOAT_32_UNSIGNED_INT_24_8_REV */
    }
    size_t component;
    switch (type) {
    case 0x1401: case 0x1400: component = 1; break;                   /* UNSIGNED_BYTE, BYTE */
    case 0x1403: case 0x1402: case 0x140B: case 0x8D61: component = 2; break;
    case 0x1405: case 0x1404: case 0x1406: component = 4; break;
    default: return 0;
    }
    switch (format) {
    case 0x1903: case 0x8D94: case 0x1906: case 0x1909: case 0x1902: case 0x1901: return component;      /* RED .. STENCIL */
    case 0x8227: case 0x8228: case 0x190A: return 2 * component;                                         /* RG, RG_INTEGER, LUMINANCE_ALPHA */
    case 0x1907: case 0x8D98: return 3 * component;                                                      /* RGB, RGB_INTEGER */
    case 0x1908: case 0x8D99: case 0x80E1: return 4 * component;                                         /* RGBA, RGBA_INTEGER, BGRA */
    }
    return 0;
}

/* The bytes a transfer reaches with the pixel store state (OpenGL ES 3.2 §8.4.4.1); 0 for a size or format the GL
 * refuses. `pack` picks the pack state; `depth` is 0 for a two-dimensional transfer. */
size_t fake_transfer(int32_t width, int32_t height, int32_t depth, uint32_t format, uint32_t type, int pack) {
    size_t pixel = pixel_bytes(format, type);
    int three = depth != 0;
    if (pixel == 0 || width <= 0 || height <= 0 || (three && depth < 0)) return 0;
    if (three && depth == 0) return 0;
    const int32_t *store = fake.store[pack];
    size_t row = (size_t)(store[1] > 0 ? store[1] : width) * pixel;
    size_t alignment = store[0] > 0 ? (size_t)store[0] : 1;
    size_t stride = (row + alignment - 1) / alignment * alignment;
    size_t rows = three && store[4] > 0 ? (size_t)store[4] : (size_t)height;
    size_t last_image = three ? (size_t)store[5] + (size_t)depth - 1 : 0;
    size_t last_row = last_image * rows + (size_t)store[2] + (size_t)height - 1;
    return last_row * stride + ((size_t)store[3] + (size_t)width) * pixel;
}

int fake_unpack_bound(void) { return fake.unpack_buffer != 0; }
int fake_pack_bound(void) { return fake.pack_buffer != 0; }

/* ----- The OpenGL ES commands whose state the glue asks about. ----- */

/* Every extension the set names. */
#define EXTENSION_COUNT 25
static const char *const extensions[EXTENSION_COUNT] = {"GL_ANGLE_polygon_mode", "GL_ANGLE_provoking_vertex", "GL_EXT_blend_func_extended", "GL_EXT_clip_control", "GL_EXT_clip_cull_distance", "GL_EXT_color_buffer_float", "GL_EXT_depth_clamp", "GL_EXT_disjoint_timer_query", "GL_EXT_float_blend", "GL_EXT_occlusion_query_boolean", "GL_EXT_polygon_offset_clamp", "GL_EXT_robustness", "GL_EXT_texture_compression_bptc", "GL_EXT_texture_compression_rgtc", "GL_EXT_texture_compression_s3tc", "GL_EXT_texture_filter_anisotropic", "GL_EXT_texture_format_BGRA8888", "GL_EXT_texture_mirror_clamp_to_edge", "GL_EXT_texture_norm16", "GL_KHR_parallel_shader_compile", "GL_KHR_texture_compression_astc_ldr", "GL_OES_draw_buffers_indexed", "GL_OES_texture_float_linear", "GL_OES_texture_half_float", "GL_OES_viewport_array"};

void fake_glBindBuffer(uint32_t target, uint32_t buffer) {
    fake_log("glBindBuffer %lld %lld", (long long)target, (long long)buffer);
    switch (target) {
    case 0x8892: fake.array_buffer = buffer; break;
    case 0x8893: fake.element_buffer = buffer; break;
    case 0x88EC: fake.unpack_buffer = buffer; break;
    case 0x88EB: fake.pack_buffer = buffer; break;
    }
}

void fake_glDeleteBuffers(int32_t count, const uint32_t *buffers) {
    fake_log("glDeleteBuffers %lld %s", (long long)count, fake_values(buffers, count > 0 ? (size_t)count : 0, 'u'));
    if (count < 0) return;
    fake_read("glDeleteBuffers", buffers, (size_t)count * sizeof *buffers);
    for (int32_t at = 0; at < count; at++) {
        uint32_t name = buffers[at];
        if (name == 0) continue;
        if (fake.array_buffer == name) fake.array_buffer = 0;
        if (fake.element_buffer == name) fake.element_buffer = 0;
        if (fake.unpack_buffer == name) fake.unpack_buffer = 0;
        if (fake.pack_buffer == name) fake.pack_buffer = 0;
        /* The attributes keep reading at their offsets, now as addresses: what the glue's draw check is for. */
        for (int index = 0; index < ATTRIBUTES; index++)
            if (fake.attributes[index].buffer == name) fake.attributes[index].buffer = 0;
    }
}

void fake_glGenBuffers(int32_t count, uint32_t *buffers) {
    fake_log("glGenBuffers %lld %s", (long long)count, FAKE_ADDRESS(buffers));
    if (count < 0) return;
    fake_write("glGenBuffers", buffers, (size_t)count * sizeof *buffers);
    for (int32_t at = 0; at < count; at++) buffers[at] = fake.next_name++;
    fake_log("  wrote 1 %s", fake_values(buffers, (size_t)count, 'u'));
}

void fake_glPixelStorei(uint32_t name, int32_t value) {
    static const uint32_t names[2][6] = {{0x0CF5, 0x0CF2, 0x0CF3, 0x0CF4, 0x806E, 0x806D}, {0x0D05, 0x0D02, 0x0D03, 0x0D04, 0, 0}};
    fake_log("glPixelStorei %lld %lld", (long long)name, (long long)value);
    if (value < 0) return;
    for (int pack = 0; pack < 2; pack++)
        for (int at = 0; at < 6; at++)
            if (names[pack][at] == name && name != 0) {
                if (at == 0 && value != 1 && value != 2 && value != 4 && value != 8) return;
                fake.store[pack][at] = value;
            }
}

void fake_glGetIntegerv(uint32_t name, int32_t *values) {
    fake_log("glGetIntegerv %lld %s", (long long)name, FAKE_ADDRESS(values));
    /* The most any name writes, as the glue's room assumes: 64. */
    fake_write("glGetIntegerv", values, 64 * sizeof *values);
    int32_t answer = (int32_t)fake_value();
    int known = 1;
    switch (name) {
    case 0x8894: answer = (int32_t)fake.array_buffer; break;
    case 0x8895: answer = (int32_t)fake.element_buffer; break;
    case 0x88EF: answer = (int32_t)fake.unpack_buffer; break;
    case 0x88ED: answer = (int32_t)fake.pack_buffer; break;
    case 0x8869: answer = fake.lies ? (int32_t)(fake_next() & 0x7FFFFFFF) : ATTRIBUTES; break; /* MAX_VERTEX_ATTRIBS */
    case 0x821D: answer = fake.lies ? (int32_t)(fake_next() & 0x7FFFFFFF) : EXTENSION_COUNT; break; /* NUM_EXTENSIONS */
    case 0x0CF5: answer = fake.store[0][0]; break;
    case 0x0CF2: answer = fake.store[0][1]; break;
    case 0x0CF3: answer = fake.store[0][2]; break;
    case 0x0CF4: answer = fake.store[0][3]; break;
    case 0x806E: answer = fake.store[0][4]; break;
    case 0x806D: answer = fake.store[0][5]; break;
    case 0x0D05: answer = fake.store[1][0]; break;
    case 0x0D02: answer = fake.store[1][1]; break;
    case 0x0D03: answer = fake.store[1][2]; break;
    case 0x0D04: answer = fake.store[1][3]; break;
    default: known = 0; break;
    }
    /* A name no state is kept for keeps the pattern written, in a logged run. */
    if (known || !fake.logging) values[0] = answer;
    fake_log("  wrote 1 %s", fake_values(values, 64, 'i'));
}

void fake_glGetVertexAttribiv(uint32_t index, uint32_t name, int32_t *values) {
    fake_log("glGetVertexAttribiv %lld %lld %s", (long long)index, (long long)name, FAKE_ADDRESS(values));
    fake_write("glGetVertexAttribiv", values, 16 * sizeof *values);
    if (index < ATTRIBUTES && name == 0x8622) values[0] = fake.attributes[index].enabled;              /* VERTEX_ATTRIB_ARRAY_ENABLED */
    if (index < ATTRIBUTES && name == 0x889F) values[0] = (int32_t)fake.attributes[index].buffer;      /* VERTEX_ATTRIB_ARRAY_BUFFER_BINDING */
    fake_log("  wrote 2 %s", fake_values(values, 16, 'i'));
}

static void attribute_pointer(uint32_t index, const void *pointer) {
    if (index >= ATTRIBUTES) return;
    fake.attributes[index].buffer = fake.array_buffer;
    fake.attributes[index].pointer = pointer;
}

void fake_glVertexAttribPointer(uint32_t index, int32_t size, uint32_t type, uint8_t normalized, int32_t stride, const void *pointer) {
    fake_log("glVertexAttribPointer %lld %lld %lld %lld %lld %lld", (long long)index, (long long)size, (long long)type, (long long)normalized, (long long)stride, (long long)(intptr_t)pointer);
    (void)size; (void)type; (void)normalized; (void)stride;
    attribute_pointer(index, pointer);
}

void fake_glVertexAttribIPointer(uint32_t index, int32_t size, uint32_t type, int32_t stride, const void *pointer) {
    fake_log("glVertexAttribIPointer %lld %lld %lld %lld %lld", (long long)index, (long long)size, (long long)type, (long long)stride, (long long)(intptr_t)pointer);
    (void)size; (void)type; (void)stride;
    attribute_pointer(index, pointer);
}

void fake_glEnableVertexAttribArray(uint32_t index) {
    fake_log("glEnableVertexAttribArray %lld", (long long)index);
    if (index < ATTRIBUTES) fake.attributes[index].enabled = 1;
}
void fake_glDisableVertexAttribArray(uint32_t index) {
    fake_log("glDisableVertexAttribArray %lld", (long long)index);
    if (index < ATTRIBUTES) fake.attributes[index].enabled = 0;
}

/* A draw reads each enabled attribute: from its buffer, or from the caller's memory at its pointer when it has none. */
static void draw(const char *command) {
    for (int index = 0; index < ATTRIBUTES; index++)
        if (fake.attributes[index].enabled && fake.attributes[index].buffer == 0) fake_read(command, fake.attributes[index].pointer, 16);
}

void fake_glDrawArrays(uint32_t mode, int32_t first, int32_t count) {
    fake_log("glDrawArrays %lld %lld %lld", (long long)mode, (long long)first, (long long)count);
    if (count > 0) draw("glDrawArrays");
}
void fake_glDrawArraysInstanced(uint32_t mode, int32_t first, int32_t count, int32_t instances) {
    fake_log("glDrawArraysInstanced %lld %lld %lld %lld", (long long)mode, (long long)first, (long long)count, (long long)instances);
    if (count > 0 && instances > 0) draw("glDrawArraysInstanced");
}
void fake_glDrawElementsInstanced(uint32_t mode, int32_t count, uint32_t type, const void *indices, int32_t instances) {
    fake_log("glDrawElementsInstanced %lld %lld %lld %lld %lld", (long long)mode, (long long)count, (long long)type, (long long)(intptr_t)indices, (long long)instances);
    if (count <= 0 || instances <= 0) return;
    size_t each = type == 0x1401 ? 1 : type == 0x1403 ? 2 : type == 0x1405 ? 4 : 0;
    if (each == 0) return;
    if (fake.element_buffer == 0) fake_read("glDrawElementsInstanced", indices, (size_t)count * each);
    draw("glDrawElementsInstanced");
}

const uint8_t *fake_glGetString(uint32_t name) {
    fake_log("glGetString %lld", (long long)name);
    if (fake.lies && fake_next() % 4 == 0) return NULL;
    const uint8_t *text = (const uint8_t *)"OpenGL ES 3.2 fake";
    fake_log("  returned %s", fake_text(text));
    return text;
}

const uint8_t *fake_glGetStringi(uint32_t name, uint32_t index) {
    fake_log("glGetStringi %lld %lld", (long long)name, (long long)index);
    if (fake.lies && fake_next() % 4 == 0) return NULL;
    const uint8_t *text = (const uint8_t *)extensions[index % EXTENSION_COUNT];
    fake_log("  returned %s", fake_text(text));
    return text;
}

void *fake_glMapBufferRange(uint32_t target, intptr_t offset, intptr_t length, uint32_t access) {
    fake_log("glMapBufferRange %lld %lld %lld %lld", (long long)target, (long long)offset, (long long)length, (long long)access);
    if (length <= 0 || length > (1 << 20) || fake.mapped || (fake.lies && fake_next() % 4 == 0)) return NULL;
    fake.mapped = malloc((size_t)length);
    if (fake.mapped) memset(fake.mapped, 0x5A, (size_t)length);
    return fake.mapped;
}

uint8_t fake_glUnmapBuffer(uint32_t target) {
    fake_log("glUnmapBuffer %lld", (long long)target);
    free(fake.mapped);
    fake.mapped = NULL;
    return 1;
}

/* A handle the library makes: a distinct address of its own (the glue passes it back only to the library); in a logged
 * run each the next of tokens (the first two are the display's). */
void *fake_handle(void) {
    fake_start();
    if (fake.logging) return &tokens[2 + fake.handles++ % (sizeof tokens - 2)];
    if (fake.lies && fake_next() % 8 == 0) return NULL;
    return &tokens[fake_next() % sizeof tokens];
}

void *fake_glFenceSync(uint32_t condition, uint32_t flags) {
    fake_log("glFenceSync %lld %lld", (long long)condition, (long long)flags);
    void *made = fake_handle();
    fake_log("  returned %s", fake_handle_name(made));
    return made;
}

uint32_t fake_glGetError(void) {
    fake_log("glGetError");
    uint32_t error = fake.lies ? (uint32_t)fake_next() : fake.logging ? (uint32_t)fake_returned() : 0;
    fake_log("  returned %lld", (long long)error);
    return error;
}

/* ----- EGL. ----- */

static void read_attributes(const char *command, const int32_t *list) {
    if (list == NULL) return;
    for (size_t at = 0;; at += 2) {
        fake_read(command, &list[at], sizeof *list);
        if (list[at] == NONE_KEY) return;
        fake_read(command, &list[at + 1], sizeof *list);
    }
}

/* An attribute list as the log writes it: every key and value up to and with its EGL_NONE. */
static const char *fake_attributes(const int32_t *list) {
    if (!fake.logging) return "";
    if (list == NULL) return "null";
    size_t count = 0;
    while (list[count] != NONE_KEY) count += 2;
    return fake_values(list, count + 1, 'i');
}

/* An EGLBoolean answer: logged, and EGL_FALSE while refusing. */
static uint32_t answered(uint32_t answer) {
    if (fake_refusing()) answer = 0;
    fake_log("  returned %lld", (long long)answer);
    return answer;
}

/* A handle answered: logged, and none while refusing. */
static void *made_handle(void) {
    void *made = fake_refusing() ? NULL : fake_handle();
    fake_log("  returned %s", fake_handle_name(made));
    return made;
}

void *eglGetProcAddress(const char *name);
void *fake_gl_lookup(const char *name);

void *eglGetDisplay(void *native) {
    fake_start();
    fake_log("eglGetDisplay %s", fake_handle_name(native));
    void *display = fake_refusing() ? NULL : &tokens[1];
    fake_log("  returned %s", fake_handle_name(display));
    return display;
}
uint32_t eglInitialize(void *display, int32_t *major, int32_t *minor) {
    fake_log("eglInitialize %s %s %s", fake_handle_name(display), FAKE_ADDRESS(major), FAKE_ADDRESS(minor));
    fake_write("eglInitialize", major, sizeof *major);
    fake_write("eglInitialize", minor, sizeof *minor);
    if (!fake.logging) {
        *major = 1;
        *minor = 5;
    }
    fake_log("  wrote 1 %s", fake_values(major, 1, 'i'));
    fake_log("  wrote 2 %s", fake_values(minor, 1, 'i'));
    return answered(1);
}
uint32_t eglBindAPI(uint32_t api) {
    fake_log("eglBindAPI %lld", (long long)api);
    return answered(1);
}
uint32_t eglChooseConfig(void *display, const int32_t *list, void **configs, int32_t room, int32_t *found) {
    fake_log("eglChooseConfig %s %s %s %lld %s", fake_handle_name(display), fake_attributes(list), FAKE_ADDRESS(configs), (long long)room, FAKE_ADDRESS(found));
    read_attributes("eglChooseConfig", list);
    fake_write("eglChooseConfig", found, sizeof *found);
    int32_t written = room < MOST_CONFIGS ? room : MOST_CONFIGS;
    if (written < 0) written = 0;
    if (configs) {
        fake_write("eglChooseConfig", configs, (size_t)written * sizeof *configs);
        for (int32_t at = 0; at < written; at++) configs[at] = fake_handle();
        fake_log("  wrote 2 %s", fake_handles(configs, (size_t)written));
    }
    /* A lie: more found than written. */
    *found = fake.lies ? (int32_t)(fake_next() & 0x7FFFFFFF) : written;
    fake_log("  wrote 4 %s", fake_values(found, 1, 'i'));
    return answered(1);
}
void *eglCreateContext(void *display, void *config, void *share, const int32_t *list) {
    fake_log("eglCreateContext %s %s %s %s", fake_handle_name(display), fake_handle_name(config), fake_handle_name(share), fake_attributes(list));
    read_attributes("eglCreateContext", list);
    return made_handle();
}
void *eglCreatePbufferSurface(void *display, void *config, const int32_t *list) {
    fake_log("eglCreatePbufferSurface %s %s %s", fake_handle_name(display), fake_handle_name(config), fake_attributes(list));
    read_attributes("eglCreatePbufferSurface", list);
    return made_handle();
}
uint32_t eglDestroyContext(void *display, void *context) {
    fake_log("eglDestroyContext %s %s", fake_handle_name(display), fake_handle_name(context));
    uint32_t answer = answered(1);
    if (answer && fake.current == context) fake.current = NULL;
    return answer;
}
uint32_t eglDestroySurface(void *display, void *surface) {
    fake_log("eglDestroySurface %s %s", fake_handle_name(display), fake_handle_name(surface));
    return answered(1);
}
uint32_t eglGetConfigAttrib(void *display, void *config, int32_t attribute, int32_t *value) {
    fake_log("eglGetConfigAttrib %s %s %lld %s", fake_handle_name(display), fake_handle_name(config), (long long)attribute, FAKE_ADDRESS(value));
    fake_write("eglGetConfigAttrib", value, sizeof *value);
    fake_log("  wrote 3 %s", fake_values(value, 1, 'i'));
    return answered(1);
}
int32_t eglGetError(void) {
    fake_log("eglGetError");
    int32_t error = fake.lies ? (int32_t)fake_next() : fake.logging ? (int32_t)fake_returned() : 0x3000;
    fake_log("  returned %lld", (long long)error);
    return error;
}
uint32_t eglMakeCurrent(void *display, void *draw_surface, void *read_surface, void *context) {
    fake_log("eglMakeCurrent %s %s %s %s", fake_handle_name(display), fake_handle_name(draw_surface), fake_handle_name(read_surface), fake_handle_name(context));
    if (fake.lies && fake_next() % 16 == 0) return 0;
    uint32_t answer = answered(1);
    if (answer) fake.current = context;
    return answer;
}
void *eglGetCurrentContext(void) { return fake.current; }
const char *eglQueryString(void *display, int32_t name) {
    fake_start();
    fake_log("eglQueryString %s %lld", fake_handle_name(display), (long long)name);
    const char *text = "fake";
    if (display == NULL) text = "EGL_KHR_client_get_all_proc_addresses";
    else if (fake_refusing() || (fake.lies && fake_next() % 4 == 0)) text = NULL;
    fake_log("  returned %s", fake_text(text));
    return text;
}
uint32_t eglQuerySurface(void *display, void *surface, int32_t attribute, int32_t *value) {
    fake_log("eglQuerySurface %s %s %lld %s", fake_handle_name(display), fake_handle_name(surface), (long long)attribute, FAKE_ADDRESS(value));
    fake_write("eglQuerySurface", value, sizeof *value);
    if (!fake.logging) *value = 4;
    fake_log("  wrote 3 %s", fake_values(value, 1, 'i'));
    return answered(1);
}
uint32_t eglSwapBuffers(void *display, void *surface) {
    fake_log("eglSwapBuffers %s %s", fake_handle_name(display), fake_handle_name(surface));
    return answered(1);
}
uint32_t eglSwapInterval(void *display, int32_t interval) {
    fake_log("eglSwapInterval %s %lld", fake_handle_name(display), (long long)interval);
    return answered(1);
}
void *eglGetProcAddress(const char *name) {
    fake_start();
    fake_read_text("eglGetProcAddress", name);
    return fake_gl_lookup(name);
}

/* ----- Every other OpenGL ES command of the set: the most its parameters reach, read or written. ----- */

static void fake_glActiveTexture(uint32_t p0) {
    fake_log("glActiveTexture %lld", (long long)p0);
    (void)fake_value();
    (void)p0;
}

static void fake_glAttachShader(uint32_t p0, uint32_t p1) {
    fake_log("glAttachShader %lld %lld", (long long)p0, (long long)p1);
    (void)fake_value();
    (void)p0;
    (void)p1;
}

static void fake_glBeginQuery(uint32_t p0, uint32_t p1) {
    fake_log("glBeginQuery %lld %lld", (long long)p0, (long long)p1);
    (void)fake_value();
    (void)p0;
    (void)p1;
}

static void fake_glBeginTransformFeedback(uint32_t p0) {
    fake_log("glBeginTransformFeedback %lld", (long long)p0);
    (void)fake_value();
    (void)p0;
}

static void fake_glBindBufferBase(uint32_t p0, uint32_t p1, uint32_t p2) {
    fake_log("glBindBufferBase %lld %lld %lld", (long long)p0, (long long)p1, (long long)p2);
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
}

static void fake_glBindBufferRange(uint32_t p0, uint32_t p1, uint32_t p2, intptr_t p3, intptr_t p4) {
    fake_log("glBindBufferRange %lld %lld %lld %lld %lld", (long long)p0, (long long)p1, (long long)p2, (long long)p3, (long long)p4);
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
    (void)p3;
    (void)p4;
}

static void fake_glBindFramebuffer(uint32_t p0, uint32_t p1) {
    fake_log("glBindFramebuffer %lld %lld", (long long)p0, (long long)p1);
    (void)fake_value();
    (void)p0;
    (void)p1;
}

static void fake_glBindRenderbuffer(uint32_t p0, uint32_t p1) {
    fake_log("glBindRenderbuffer %lld %lld", (long long)p0, (long long)p1);
    (void)fake_value();
    (void)p0;
    (void)p1;
}

static void fake_glBindSampler(uint32_t p0, uint32_t p1) {
    fake_log("glBindSampler %lld %lld", (long long)p0, (long long)p1);
    (void)fake_value();
    (void)p0;
    (void)p1;
}

static void fake_glBindTexture(uint32_t p0, uint32_t p1) {
    fake_log("glBindTexture %lld %lld", (long long)p0, (long long)p1);
    (void)fake_value();
    (void)p0;
    (void)p1;
}

static void fake_glBindVertexArray(uint32_t p0) {
    fake_log("glBindVertexArray %lld", (long long)p0);
    (void)fake_value();
    (void)p0;
}

static void fake_glBlendColor(float p0, float p1, float p2, float p3) {
    fake_log("glBlendColor %.9g %.9g %.9g %.9g", (double)p0, (double)p1, (double)p2, (double)p3);
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
    (void)p3;
}

static void fake_glBlendEquationSeparate(uint32_t p0, uint32_t p1) {
    fake_log("glBlendEquationSeparate %lld %lld", (long long)p0, (long long)p1);
    (void)fake_value();
    (void)p0;
    (void)p1;
}

static void fake_glBlendFuncSeparate(uint32_t p0, uint32_t p1, uint32_t p2, uint32_t p3) {
    fake_log("glBlendFuncSeparate %lld %lld %lld %lld", (long long)p0, (long long)p1, (long long)p2, (long long)p3);
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
    (void)p3;
}

static void fake_glBlitFramebuffer(int32_t p0, int32_t p1, int32_t p2, int32_t p3, int32_t p4, int32_t p5, int32_t p6, int32_t p7, uint32_t p8, uint32_t p9) {
    fake_log("glBlitFramebuffer %lld %lld %lld %lld %lld %lld %lld %lld %lld %lld", (long long)p0, (long long)p1, (long long)p2, (long long)p3, (long long)p4, (long long)p5, (long long)p6, (long long)p7, (long long)p8, (long long)p9);
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
    (void)p3;
    (void)p4;
    (void)p5;
    (void)p6;
    (void)p7;
    (void)p8;
    (void)p9;
}

static void fake_glBufferData(uint32_t p0, intptr_t p1, const void * p2, uint32_t p3) {
    fake_log("glBufferData %lld %lld %s %lld", (long long)p0, (long long)p1, fake_bytes(p2, p1 > 0 ? (size_t)p1 : 0), (long long)p3);
    if (p2 && p1 > 0) fake_read("glBufferData", p2, (size_t)p1);
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
    (void)p3;
}

static void fake_glBufferSubData(uint32_t p0, intptr_t p1, intptr_t p2, const void * p3) {
    fake_log("glBufferSubData %lld %lld %lld %s", (long long)p0, (long long)p1, (long long)p2, fake_bytes(p3, (p2 > 0 ? (size_t)p2 * 1 : 0)));
    if (p2 > 0) fake_read("glBufferSubData", p3, (size_t)p2 * 1 * 1);
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
    (void)p3;
}

static uint32_t fake_glCheckFramebufferStatus(uint32_t p0) {
    fake_log("glCheckFramebufferStatus %lld", (long long)p0);
    (void)fake_value();
    (void)p0;
    uint32_t value = fake.logging ? (uint32_t)fake_returned() : (uint32_t)fake_value();
    fake_log("  returned %lld", (long long)value);
    return value;
}

static void fake_glClearBufferfi(uint32_t p0, int32_t p1, float p2, int32_t p3) {
    fake_log("glClearBufferfi %lld %lld %.9g %lld", (long long)p0, (long long)p1, (double)p2, (long long)p3);
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
    (void)p3;
}

static void fake_glClearBufferfv(uint32_t p0, int32_t p1, const float * p2) {
    fake_log("glClearBufferfv %lld %lld %s", (long long)p0, (long long)p1, fake_values(p2, 4, 'f'));
    fake_read("glClearBufferfv", p2, 4 * sizeof(float));
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
}

static void fake_glClearBufferiv(uint32_t p0, int32_t p1, const int32_t * p2) {
    fake_log("glClearBufferiv %lld %lld %s", (long long)p0, (long long)p1, fake_values(p2, 4, 'i'));
    fake_read("glClearBufferiv", p2, 4 * sizeof(int32_t));
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
}

static void fake_glClearBufferuiv(uint32_t p0, int32_t p1, const uint32_t * p2) {
    fake_log("glClearBufferuiv %lld %lld %s", (long long)p0, (long long)p1, fake_values(p2, 4, 'u'));
    fake_read("glClearBufferuiv", p2, 4 * sizeof(uint32_t));
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
}

static uint32_t fake_glClientWaitSync(void * p0, uint32_t p1, uint64_t p2) {
    fake_log("glClientWaitSync %s %lld %llu", fake_handle_name(p0), (long long)p1, (unsigned long long)p2);
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
    uint32_t value = fake.logging ? (uint32_t)fake_returned() : (uint32_t)fake_value();
    fake_log("  returned %lld", (long long)value);
    return value;
}

static void fake_glColorMask(uint8_t p0, uint8_t p1, uint8_t p2, uint8_t p3) {
    fake_log("glColorMask %lld %lld %lld %lld", (long long)p0, (long long)p1, (long long)p2, (long long)p3);
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
    (void)p3;
}

static void fake_glCompileShader(uint32_t p0) {
    fake_log("glCompileShader %lld", (long long)p0);
    (void)fake_value();
    (void)p0;
}

static void fake_glCompressedTexSubImage2D(uint32_t p0, int32_t p1, int32_t p2, int32_t p3, int32_t p4, int32_t p5, uint32_t p6, int32_t p7, const void * p8) {
    fake_log("glCompressedTexSubImage2D %lld %lld %lld %lld %lld %lld %lld %lld %s", (long long)p0, (long long)p1, (long long)p2, (long long)p3, (long long)p4, (long long)p5, (long long)p6, (long long)p7, fake_unpack_bound() ? "offset" : fake_bytes(p8, (p7 > 0 ? (size_t)p7 * 1 : 0)));
    if (!fake_unpack_bound() && p7 > 0) fake_read("glCompressedTexSubImage2D", p8, (size_t)p7);
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
    (void)p3;
    (void)p4;
    (void)p5;
    (void)p6;
    (void)p7;
    (void)p8;
}

static void fake_glCompressedTexSubImage3D(uint32_t p0, int32_t p1, int32_t p2, int32_t p3, int32_t p4, int32_t p5, int32_t p6, int32_t p7, uint32_t p8, int32_t p9, const void * p10) {
    fake_log("glCompressedTexSubImage3D %lld %lld %lld %lld %lld %lld %lld %lld %lld %lld %s", (long long)p0, (long long)p1, (long long)p2, (long long)p3, (long long)p4, (long long)p5, (long long)p6, (long long)p7, (long long)p8, (long long)p9, fake_unpack_bound() ? "offset" : fake_bytes(p10, (p9 > 0 ? (size_t)p9 * 1 : 0)));
    if (!fake_unpack_bound() && p9 > 0) fake_read("glCompressedTexSubImage3D", p10, (size_t)p9);
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
    (void)p3;
    (void)p4;
    (void)p5;
    (void)p6;
    (void)p7;
    (void)p8;
    (void)p9;
    (void)p10;
}

static void fake_glCopyBufferSubData(uint32_t p0, uint32_t p1, intptr_t p2, intptr_t p3, intptr_t p4) {
    fake_log("glCopyBufferSubData %lld %lld %lld %lld %lld", (long long)p0, (long long)p1, (long long)p2, (long long)p3, (long long)p4);
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
    (void)p3;
    (void)p4;
}

static void fake_glCopyTexSubImage2D(uint32_t p0, int32_t p1, int32_t p2, int32_t p3, int32_t p4, int32_t p5, int32_t p6, int32_t p7) {
    fake_log("glCopyTexSubImage2D %lld %lld %lld %lld %lld %lld %lld %lld", (long long)p0, (long long)p1, (long long)p2, (long long)p3, (long long)p4, (long long)p5, (long long)p6, (long long)p7);
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
    (void)p3;
    (void)p4;
    (void)p5;
    (void)p6;
    (void)p7;
}

static void fake_glCopyTexSubImage3D(uint32_t p0, int32_t p1, int32_t p2, int32_t p3, int32_t p4, int32_t p5, int32_t p6, int32_t p7, int32_t p8) {
    fake_log("glCopyTexSubImage3D %lld %lld %lld %lld %lld %lld %lld %lld %lld", (long long)p0, (long long)p1, (long long)p2, (long long)p3, (long long)p4, (long long)p5, (long long)p6, (long long)p7, (long long)p8);
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
    (void)p3;
    (void)p4;
    (void)p5;
    (void)p6;
    (void)p7;
    (void)p8;
}

static uint32_t fake_glCreateProgram(void) {
    fake_log("glCreateProgram");
    (void)fake_value();
    uint32_t value = fake.logging ? (uint32_t)fake_returned() : (uint32_t)fake_value();
    fake_log("  returned %lld", (long long)value);
    return value;
}

static uint32_t fake_glCreateShader(uint32_t p0) {
    fake_log("glCreateShader %lld", (long long)p0);
    (void)fake_value();
    (void)p0;
    uint32_t value = fake.logging ? (uint32_t)fake_returned() : (uint32_t)fake_value();
    fake_log("  returned %lld", (long long)value);
    return value;
}

static void fake_glCullFace(uint32_t p0) {
    fake_log("glCullFace %lld", (long long)p0);
    (void)fake_value();
    (void)p0;
}

static void fake_glDeleteFramebuffers(int32_t p0, const uint32_t * p1) {
    fake_log("glDeleteFramebuffers %lld %s", (long long)p0, fake_values(p1, (p0 > 0 ? (size_t)p0 * 1 : 0), 'u'));
    if (p0 > 0) fake_read("glDeleteFramebuffers", p1, (size_t)p0 * 1 * sizeof(uint32_t));
    (void)fake_value();
    (void)p0;
    (void)p1;
}

static void fake_glDeleteProgram(uint32_t p0) {
    fake_log("glDeleteProgram %lld", (long long)p0);
    (void)fake_value();
    (void)p0;
}

static void fake_glDeleteQueries(int32_t p0, const uint32_t * p1) {
    fake_log("glDeleteQueries %lld %s", (long long)p0, fake_values(p1, (p0 > 0 ? (size_t)p0 * 1 : 0), 'u'));
    if (p0 > 0) fake_read("glDeleteQueries", p1, (size_t)p0 * 1 * sizeof(uint32_t));
    (void)fake_value();
    (void)p0;
    (void)p1;
}

static void fake_glDeleteRenderbuffers(int32_t p0, const uint32_t * p1) {
    fake_log("glDeleteRenderbuffers %lld %s", (long long)p0, fake_values(p1, (p0 > 0 ? (size_t)p0 * 1 : 0), 'u'));
    if (p0 > 0) fake_read("glDeleteRenderbuffers", p1, (size_t)p0 * 1 * sizeof(uint32_t));
    (void)fake_value();
    (void)p0;
    (void)p1;
}

static void fake_glDeleteSamplers(int32_t p0, const uint32_t * p1) {
    fake_log("glDeleteSamplers %lld %s", (long long)p0, fake_values(p1, (p0 > 0 ? (size_t)p0 * 1 : 0), 'u'));
    if (p0 > 0) fake_read("glDeleteSamplers", p1, (size_t)p0 * 1 * sizeof(uint32_t));
    (void)fake_value();
    (void)p0;
    (void)p1;
}

static void fake_glDeleteShader(uint32_t p0) {
    fake_log("glDeleteShader %lld", (long long)p0);
    (void)fake_value();
    (void)p0;
}

static void fake_glDeleteSync(void * p0) {
    fake_log("glDeleteSync %s", fake_handle_name(p0));
    (void)fake_value();
    (void)p0;
}

static void fake_glDeleteTextures(int32_t p0, const uint32_t * p1) {
    fake_log("glDeleteTextures %lld %s", (long long)p0, fake_values(p1, (p0 > 0 ? (size_t)p0 * 1 : 0), 'u'));
    if (p0 > 0) fake_read("glDeleteTextures", p1, (size_t)p0 * 1 * sizeof(uint32_t));
    (void)fake_value();
    (void)p0;
    (void)p1;
}

static void fake_glDeleteVertexArrays(int32_t p0, const uint32_t * p1) {
    fake_log("glDeleteVertexArrays %lld %s", (long long)p0, fake_values(p1, (p0 > 0 ? (size_t)p0 * 1 : 0), 'u'));
    if (p0 > 0) fake_read("glDeleteVertexArrays", p1, (size_t)p0 * 1 * sizeof(uint32_t));
    (void)fake_value();
    (void)p0;
    (void)p1;
}

static void fake_glDepthFunc(uint32_t p0) {
    fake_log("glDepthFunc %lld", (long long)p0);
    (void)fake_value();
    (void)p0;
}

static void fake_glDepthMask(uint8_t p0) {
    fake_log("glDepthMask %lld", (long long)p0);
    (void)fake_value();
    (void)p0;
}

static void fake_glDepthRangef(float p0, float p1) {
    fake_log("glDepthRangef %.9g %.9g", (double)p0, (double)p1);
    (void)fake_value();
    (void)p0;
    (void)p1;
}

static void fake_glDetachShader(uint32_t p0, uint32_t p1) {
    fake_log("glDetachShader %lld %lld", (long long)p0, (long long)p1);
    (void)fake_value();
    (void)p0;
    (void)p1;
}

static void fake_glDisable(uint32_t p0) {
    fake_log("glDisable %lld", (long long)p0);
    (void)fake_value();
    (void)p0;
}

static void fake_glDrawBuffers(int32_t p0, const uint32_t * p1) {
    fake_log("glDrawBuffers %lld %s", (long long)p0, fake_values(p1, (p0 > 0 ? (size_t)p0 * 1 : 0), 'u'));
    if (p0 > 0) fake_read("glDrawBuffers", p1, (size_t)p0 * 1 * sizeof(uint32_t));
    (void)fake_value();
    (void)p0;
    (void)p1;
}

static void fake_glEnable(uint32_t p0) {
    fake_log("glEnable %lld", (long long)p0);
    (void)fake_value();
    (void)p0;
}

static void fake_glEndQuery(uint32_t p0) {
    fake_log("glEndQuery %lld", (long long)p0);
    (void)fake_value();
    (void)p0;
}

static void fake_glEndTransformFeedback(void) {
    fake_log("glEndTransformFeedback");
    (void)fake_value();
}

static void fake_glFinish(void) {
    fake_log("glFinish");
    (void)fake_value();
}

static void fake_glFlush(void) {
    fake_log("glFlush");
    (void)fake_value();
}

static void fake_glFramebufferRenderbuffer(uint32_t p0, uint32_t p1, uint32_t p2, uint32_t p3) {
    fake_log("glFramebufferRenderbuffer %lld %lld %lld %lld", (long long)p0, (long long)p1, (long long)p2, (long long)p3);
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
    (void)p3;
}

static void fake_glFramebufferTexture2D(uint32_t p0, uint32_t p1, uint32_t p2, uint32_t p3, int32_t p4) {
    fake_log("glFramebufferTexture2D %lld %lld %lld %lld %lld", (long long)p0, (long long)p1, (long long)p2, (long long)p3, (long long)p4);
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
    (void)p3;
    (void)p4;
}

static void fake_glFramebufferTextureLayer(uint32_t p0, uint32_t p1, uint32_t p2, int32_t p3, int32_t p4) {
    fake_log("glFramebufferTextureLayer %lld %lld %lld %lld %lld", (long long)p0, (long long)p1, (long long)p2, (long long)p3, (long long)p4);
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
    (void)p3;
    (void)p4;
}

static void fake_glFrontFace(uint32_t p0) {
    fake_log("glFrontFace %lld", (long long)p0);
    (void)fake_value();
    (void)p0;
}

static void fake_glGenFramebuffers(int32_t p0, uint32_t * p1) {
    fake_log("glGenFramebuffers %lld %s", (long long)p0, FAKE_ADDRESS(p1));
    if (p0 > 0) fake_write("glGenFramebuffers", p1, (size_t)p0 * 1 * sizeof(uint32_t));
    fake_log("  wrote 1 %s", fake_values(p1, (p0 > 0 ? (size_t)p0 * 1 : 0), 'u'));
    (void)fake_value();
    (void)p0;
    (void)p1;
}

static void fake_glGenQueries(int32_t p0, uint32_t * p1) {
    fake_log("glGenQueries %lld %s", (long long)p0, FAKE_ADDRESS(p1));
    if (p0 > 0) fake_write("glGenQueries", p1, (size_t)p0 * 1 * sizeof(uint32_t));
    fake_log("  wrote 1 %s", fake_values(p1, (p0 > 0 ? (size_t)p0 * 1 : 0), 'u'));
    (void)fake_value();
    (void)p0;
    (void)p1;
}

static void fake_glGenRenderbuffers(int32_t p0, uint32_t * p1) {
    fake_log("glGenRenderbuffers %lld %s", (long long)p0, FAKE_ADDRESS(p1));
    if (p0 > 0) fake_write("glGenRenderbuffers", p1, (size_t)p0 * 1 * sizeof(uint32_t));
    fake_log("  wrote 1 %s", fake_values(p1, (p0 > 0 ? (size_t)p0 * 1 : 0), 'u'));
    (void)fake_value();
    (void)p0;
    (void)p1;
}

static void fake_glGenSamplers(int32_t p0, uint32_t * p1) {
    fake_log("glGenSamplers %lld %s", (long long)p0, FAKE_ADDRESS(p1));
    if (p0 > 0) fake_write("glGenSamplers", p1, (size_t)p0 * 1 * sizeof(uint32_t));
    fake_log("  wrote 1 %s", fake_values(p1, (p0 > 0 ? (size_t)p0 * 1 : 0), 'u'));
    (void)fake_value();
    (void)p0;
    (void)p1;
}

static void fake_glGenTextures(int32_t p0, uint32_t * p1) {
    fake_log("glGenTextures %lld %s", (long long)p0, FAKE_ADDRESS(p1));
    if (p0 > 0) fake_write("glGenTextures", p1, (size_t)p0 * 1 * sizeof(uint32_t));
    fake_log("  wrote 1 %s", fake_values(p1, (p0 > 0 ? (size_t)p0 * 1 : 0), 'u'));
    (void)fake_value();
    (void)p0;
    (void)p1;
}

static void fake_glGenVertexArrays(int32_t p0, uint32_t * p1) {
    fake_log("glGenVertexArrays %lld %s", (long long)p0, FAKE_ADDRESS(p1));
    if (p0 > 0) fake_write("glGenVertexArrays", p1, (size_t)p0 * 1 * sizeof(uint32_t));
    fake_log("  wrote 1 %s", fake_values(p1, (p0 > 0 ? (size_t)p0 * 1 : 0), 'u'));
    (void)fake_value();
    (void)p0;
    (void)p1;
}

static void fake_glGenerateMipmap(uint32_t p0) {
    fake_log("glGenerateMipmap %lld", (long long)p0);
    (void)fake_value();
    (void)p0;
}

static void fake_glGetBooleanv(uint32_t p0, uint8_t * p1) {
    fake_log("glGetBooleanv %lld %s", (long long)p0, FAKE_ADDRESS(p1));
    fake_write("glGetBooleanv", p1, 64 * sizeof(uint8_t));
    fake_log("  wrote 1 %s", fake_values(p1, 64, 'b'));
    (void)fake_value();
    (void)p0;
    (void)p1;
}

static void fake_glGetFloatv(uint32_t p0, float * p1) {
    fake_log("glGetFloatv %lld %s", (long long)p0, FAKE_ADDRESS(p1));
    fake_write("glGetFloatv", p1, 64 * sizeof(float));
    fake_log("  wrote 1 %s", fake_values(p1, 64, 'f'));
    (void)fake_value();
    (void)p0;
    (void)p1;
}

static void fake_glGetInteger64v(uint32_t p0, int64_t * p1) {
    fake_log("glGetInteger64v %lld %s", (long long)p0, FAKE_ADDRESS(p1));
    fake_write("glGetInteger64v", p1, 64 * sizeof(int64_t));
    fake_log("  wrote 1 %s", fake_values(p1, 64, 'I'));
    (void)fake_value();
    (void)p0;
    (void)p1;
}

static void fake_glGetInternalformativ(uint32_t p0, uint32_t p1, uint32_t p2, int32_t p3, int32_t * p4) {
    fake_log("glGetInternalformativ %lld %lld %lld %lld %s", (long long)p0, (long long)p1, (long long)p2, (long long)p3, FAKE_ADDRESS(p4));
    if (p3 > 0) fake_write("glGetInternalformativ", p4, (size_t)p3 * 1 * sizeof(int32_t));
    fake_log("  wrote 4 %s", fake_values(p4, (p3 > 0 ? (size_t)p3 * 1 : 0), 'i'));
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
    (void)p3;
    (void)p4;
}

static void fake_glGetProgramInfoLog(uint32_t p0, int32_t p1, int32_t * p2, uint8_t * p3) {
    fake_log("glGetProgramInfoLog %lld %lld %s %s", (long long)p0, (long long)p1, FAKE_ADDRESS(p2), FAKE_ADDRESS(p3));
    fake_write("glGetProgramInfoLog", p2, sizeof(int32_t));
    if (p1 > 0) fake_write("glGetProgramInfoLog", p3, (size_t)p1 * 1 * sizeof(uint8_t));
    fake_log("  wrote 2 %s", fake_values(p2, 1, 'i'));
    fake_log("  wrote 3 %s", fake_values(p3, (p1 > 0 ? (size_t)p1 * 1 : 0), 'b'));
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
    (void)p3;
}

static void fake_glGetProgramiv(uint32_t p0, uint32_t p1, int32_t * p2) {
    fake_log("glGetProgramiv %lld %lld %s", (long long)p0, (long long)p1, FAKE_ADDRESS(p2));
    fake_write("glGetProgramiv", p2, 16 * sizeof(int32_t));
    fake_log("  wrote 2 %s", fake_values(p2, 16, 'i'));
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
}

static void fake_glGetQueryObjectuiv(uint32_t p0, uint32_t p1, uint32_t * p2) {
    fake_log("glGetQueryObjectuiv %lld %lld %s", (long long)p0, (long long)p1, FAKE_ADDRESS(p2));
    fake_write("glGetQueryObjectuiv", p2, 16 * sizeof(uint32_t));
    fake_log("  wrote 2 %s", fake_values(p2, 16, 'u'));
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
}

static void fake_glGetShaderInfoLog(uint32_t p0, int32_t p1, int32_t * p2, uint8_t * p3) {
    fake_log("glGetShaderInfoLog %lld %lld %s %s", (long long)p0, (long long)p1, FAKE_ADDRESS(p2), FAKE_ADDRESS(p3));
    fake_write("glGetShaderInfoLog", p2, sizeof(int32_t));
    if (p1 > 0) fake_write("glGetShaderInfoLog", p3, (size_t)p1 * 1 * sizeof(uint8_t));
    fake_log("  wrote 2 %s", fake_values(p2, 1, 'i'));
    fake_log("  wrote 3 %s", fake_values(p3, (p1 > 0 ? (size_t)p1 * 1 : 0), 'b'));
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
    (void)p3;
}

static void fake_glGetShaderiv(uint32_t p0, uint32_t p1, int32_t * p2) {
    fake_log("glGetShaderiv %lld %lld %s", (long long)p0, (long long)p1, FAKE_ADDRESS(p2));
    fake_write("glGetShaderiv", p2, 16 * sizeof(int32_t));
    fake_log("  wrote 2 %s", fake_values(p2, 16, 'i'));
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
}

static void fake_glGetSynciv(void * p0, uint32_t p1, int32_t p2, int32_t * p3, int32_t * p4) {
    fake_log("glGetSynciv %s %lld %lld %s %s", fake_handle_name(p0), (long long)p1, (long long)p2, FAKE_ADDRESS(p3), FAKE_ADDRESS(p4));
    fake_write("glGetSynciv", p3, sizeof(int32_t));
    if (p2 > 0) fake_write("glGetSynciv", p4, (size_t)p2 * 1 * sizeof(int32_t));
    fake_log("  wrote 3 %s", fake_values(p3, 1, 'i'));
    fake_log("  wrote 4 %s", fake_values(p4, (p2 > 0 ? (size_t)p2 * 1 : 0), 'i'));
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
    (void)p3;
    (void)p4;
}

static uint32_t fake_glGetUniformBlockIndex(uint32_t p0, const uint8_t * p1) {
    fake_log("glGetUniformBlockIndex %lld %s", (long long)p0, fake_text(p1));
    fake_read_text("glGetUniformBlockIndex", p1);
    (void)fake_value();
    (void)p0;
    (void)p1;
    uint32_t value = fake.logging ? (uint32_t)fake_returned() : (uint32_t)fake_value();
    fake_log("  returned %lld", (long long)value);
    return value;
}

static int32_t fake_glGetUniformLocation(uint32_t p0, const uint8_t * p1) {
    fake_log("glGetUniformLocation %lld %s", (long long)p0, fake_text(p1));
    fake_read_text("glGetUniformLocation", p1);
    (void)fake_value();
    (void)p0;
    (void)p1;
    int32_t value = fake.logging ? (int32_t)fake_returned() : (int32_t)fake_value();
    fake_log("  returned %lld", (long long)value);
    return value;
}

static void fake_glInvalidateFramebuffer(uint32_t p0, int32_t p1, const uint32_t * p2) {
    fake_log("glInvalidateFramebuffer %lld %lld %s", (long long)p0, (long long)p1, fake_values(p2, (p1 > 0 ? (size_t)p1 * 1 : 0), 'u'));
    if (p1 > 0) fake_read("glInvalidateFramebuffer", p2, (size_t)p1 * 1 * sizeof(uint32_t));
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
}

static void fake_glLinkProgram(uint32_t p0) {
    fake_log("glLinkProgram %lld", (long long)p0);
    (void)fake_value();
    (void)p0;
}

static void fake_glPolygonOffset(float p0, float p1) {
    fake_log("glPolygonOffset %.9g %.9g", (double)p0, (double)p1);
    (void)fake_value();
    (void)p0;
    (void)p1;
}

static void fake_glReadPixels(int32_t p0, int32_t p1, int32_t p2, int32_t p3, uint32_t p4, uint32_t p5, void * p6) {
    fake_log("glReadPixels %lld %lld %lld %lld %lld %lld %s", (long long)p0, (long long)p1, (long long)p2, (long long)p3, (long long)p4, (long long)p5, FAKE_ADDRESS(p6));
    if (!fake_pack_bound()) fake_write("glReadPixels", p6, fake_transfer(p2, p3, 0, p4, p5, 1));
    if (!fake_pack_bound()) fake_log("  wrote 6 %s", fake_bytes(p6, fake_transfer(p2, p3, 0, p4, p5, 1)));
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
    (void)p3;
    (void)p4;
    (void)p5;
    (void)p6;
}

static void fake_glRenderbufferStorage(uint32_t p0, uint32_t p1, int32_t p2, int32_t p3) {
    fake_log("glRenderbufferStorage %lld %lld %lld %lld", (long long)p0, (long long)p1, (long long)p2, (long long)p3);
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
    (void)p3;
}

static void fake_glRenderbufferStorageMultisample(uint32_t p0, int32_t p1, uint32_t p2, int32_t p3, int32_t p4) {
    fake_log("glRenderbufferStorageMultisample %lld %lld %lld %lld %lld", (long long)p0, (long long)p1, (long long)p2, (long long)p3, (long long)p4);
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
    (void)p3;
    (void)p4;
}

static void fake_glSamplerParameterf(uint32_t p0, uint32_t p1, float p2) {
    fake_log("glSamplerParameterf %lld %lld %.9g", (long long)p0, (long long)p1, (double)p2);
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
}

static void fake_glSamplerParameteri(uint32_t p0, uint32_t p1, int32_t p2) {
    fake_log("glSamplerParameteri %lld %lld %lld", (long long)p0, (long long)p1, (long long)p2);
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
}

static void fake_glScissor(int32_t p0, int32_t p1, int32_t p2, int32_t p3) {
    fake_log("glScissor %lld %lld %lld %lld", (long long)p0, (long long)p1, (long long)p2, (long long)p3);
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
    (void)p3;
}

static void fake_glShaderSource(uint32_t p0, int32_t p1, const uint8_t *const * p2, const int32_t * p3) {
    fake_log("glShaderSource %lld %lld %s %s", (long long)p0, (long long)p1, fake_texts(p2, p1, p3), fake_values(p3, (p1 > 0 ? (size_t)p1 : 0), 'i'));
    for (int32_t k = 0; k < p1; k++) { fake_read("glShaderSource", &p2[k], sizeof *p2); fake_read("glShaderSource", &p3[k], sizeof *p3); if (p3[k] < 0) fake_read_text("glShaderSource", p2[k]); else fake_read("glShaderSource", p2[k], (size_t)p3[k]); }
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
    (void)p3;
}

static void fake_glStencilFunc(uint32_t p0, int32_t p1, uint32_t p2) {
    fake_log("glStencilFunc %lld %lld %lld", (long long)p0, (long long)p1, (long long)p2);
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
}

static void fake_glStencilFuncSeparate(uint32_t p0, uint32_t p1, int32_t p2, uint32_t p3) {
    fake_log("glStencilFuncSeparate %lld %lld %lld %lld", (long long)p0, (long long)p1, (long long)p2, (long long)p3);
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
    (void)p3;
}

static void fake_glStencilMask(uint32_t p0) {
    fake_log("glStencilMask %lld", (long long)p0);
    (void)fake_value();
    (void)p0;
}

static void fake_glStencilOp(uint32_t p0, uint32_t p1, uint32_t p2) {
    fake_log("glStencilOp %lld %lld %lld", (long long)p0, (long long)p1, (long long)p2);
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
}

static void fake_glStencilOpSeparate(uint32_t p0, uint32_t p1, uint32_t p2, uint32_t p3) {
    fake_log("glStencilOpSeparate %lld %lld %lld %lld", (long long)p0, (long long)p1, (long long)p2, (long long)p3);
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
    (void)p3;
}

static void fake_glTexParameteri(uint32_t p0, uint32_t p1, int32_t p2) {
    fake_log("glTexParameteri %lld %lld %lld", (long long)p0, (long long)p1, (long long)p2);
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
}

static void fake_glTexStorage2D(uint32_t p0, int32_t p1, uint32_t p2, int32_t p3, int32_t p4) {
    fake_log("glTexStorage2D %lld %lld %lld %lld %lld", (long long)p0, (long long)p1, (long long)p2, (long long)p3, (long long)p4);
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
    (void)p3;
    (void)p4;
}

static void fake_glTexStorage3D(uint32_t p0, int32_t p1, uint32_t p2, int32_t p3, int32_t p4, int32_t p5) {
    fake_log("glTexStorage3D %lld %lld %lld %lld %lld %lld", (long long)p0, (long long)p1, (long long)p2, (long long)p3, (long long)p4, (long long)p5);
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
    (void)p3;
    (void)p4;
    (void)p5;
}

static void fake_glTexSubImage2D(uint32_t p0, int32_t p1, int32_t p2, int32_t p3, int32_t p4, int32_t p5, uint32_t p6, uint32_t p7, const void * p8) {
    fake_log("glTexSubImage2D %lld %lld %lld %lld %lld %lld %lld %lld %s", (long long)p0, (long long)p1, (long long)p2, (long long)p3, (long long)p4, (long long)p5, (long long)p6, (long long)p7, fake_unpack_bound() ? "offset" : fake_bytes(p8, fake_transfer(p4, p5, 0, p6, p7, 0)));
    if (!fake_unpack_bound()) fake_read("glTexSubImage2D", p8, fake_transfer(p4, p5, 0, p6, p7, 0));
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
    (void)p3;
    (void)p4;
    (void)p5;
    (void)p6;
    (void)p7;
    (void)p8;
}

static void fake_glTexSubImage3D(uint32_t p0, int32_t p1, int32_t p2, int32_t p3, int32_t p4, int32_t p5, int32_t p6, int32_t p7, uint32_t p8, uint32_t p9, const void * p10) {
    fake_log("glTexSubImage3D %lld %lld %lld %lld %lld %lld %lld %lld %lld %lld %s", (long long)p0, (long long)p1, (long long)p2, (long long)p3, (long long)p4, (long long)p5, (long long)p6, (long long)p7, (long long)p8, (long long)p9, fake_unpack_bound() ? "offset" : fake_bytes(p10, fake_transfer(p5, p6, p7, p8, p9, 0)));
    if (!fake_unpack_bound()) fake_read("glTexSubImage3D", p10, fake_transfer(p5, p6, p7, p8, p9, 0));
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
    (void)p3;
    (void)p4;
    (void)p5;
    (void)p6;
    (void)p7;
    (void)p8;
    (void)p9;
    (void)p10;
}

static void fake_glTransformFeedbackVaryings(uint32_t p0, int32_t p1, const uint8_t *const * p2, uint32_t p3) {
    fake_log("glTransformFeedbackVaryings %lld %lld %s %lld", (long long)p0, (long long)p1, fake_texts(p2, p1, NULL), (long long)p3);
    for (int32_t k = 0; k < p1; k++) { fake_read("glTransformFeedbackVaryings", &p2[k], sizeof *p2); fake_read_text("glTransformFeedbackVaryings", p2[k]); }
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
    (void)p3;
}

static void fake_glUniform1f(int32_t p0, float p1) {
    fake_log("glUniform1f %lld %.9g", (long long)p0, (double)p1);
    (void)fake_value();
    (void)p0;
    (void)p1;
}

static void fake_glUniform1fv(int32_t p0, int32_t p1, const float * p2) {
    fake_log("glUniform1fv %lld %lld %s", (long long)p0, (long long)p1, fake_values(p2, (p1 > 0 ? (size_t)p1 * 1 : 0), 'f'));
    if (p1 > 0) fake_read("glUniform1fv", p2, (size_t)p1 * 1 * sizeof(float));
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
}

static void fake_glUniform1i(int32_t p0, int32_t p1) {
    fake_log("glUniform1i %lld %lld", (long long)p0, (long long)p1);
    (void)fake_value();
    (void)p0;
    (void)p1;
}

static void fake_glUniform1iv(int32_t p0, int32_t p1, const int32_t * p2) {
    fake_log("glUniform1iv %lld %lld %s", (long long)p0, (long long)p1, fake_values(p2, (p1 > 0 ? (size_t)p1 * 1 : 0), 'i'));
    if (p1 > 0) fake_read("glUniform1iv", p2, (size_t)p1 * 1 * sizeof(int32_t));
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
}

static void fake_glUniform1ui(int32_t p0, uint32_t p1) {
    fake_log("glUniform1ui %lld %lld", (long long)p0, (long long)p1);
    (void)fake_value();
    (void)p0;
    (void)p1;
}

static void fake_glUniform1uiv(int32_t p0, int32_t p1, const uint32_t * p2) {
    fake_log("glUniform1uiv %lld %lld %s", (long long)p0, (long long)p1, fake_values(p2, (p1 > 0 ? (size_t)p1 * 1 : 0), 'u'));
    if (p1 > 0) fake_read("glUniform1uiv", p2, (size_t)p1 * 1 * sizeof(uint32_t));
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
}

static void fake_glUniform2f(int32_t p0, float p1, float p2) {
    fake_log("glUniform2f %lld %.9g %.9g", (long long)p0, (double)p1, (double)p2);
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
}

static void fake_glUniform2fv(int32_t p0, int32_t p1, const float * p2) {
    fake_log("glUniform2fv %lld %lld %s", (long long)p0, (long long)p1, fake_values(p2, (p1 > 0 ? (size_t)p1 * 2 : 0), 'f'));
    if (p1 > 0) fake_read("glUniform2fv", p2, (size_t)p1 * 2 * sizeof(float));
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
}

static void fake_glUniform2iv(int32_t p0, int32_t p1, const int32_t * p2) {
    fake_log("glUniform2iv %lld %lld %s", (long long)p0, (long long)p1, fake_values(p2, (p1 > 0 ? (size_t)p1 * 2 : 0), 'i'));
    if (p1 > 0) fake_read("glUniform2iv", p2, (size_t)p1 * 2 * sizeof(int32_t));
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
}

static void fake_glUniform2uiv(int32_t p0, int32_t p1, const uint32_t * p2) {
    fake_log("glUniform2uiv %lld %lld %s", (long long)p0, (long long)p1, fake_values(p2, (p1 > 0 ? (size_t)p1 * 2 : 0), 'u'));
    if (p1 > 0) fake_read("glUniform2uiv", p2, (size_t)p1 * 2 * sizeof(uint32_t));
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
}

static void fake_glUniform3fv(int32_t p0, int32_t p1, const float * p2) {
    fake_log("glUniform3fv %lld %lld %s", (long long)p0, (long long)p1, fake_values(p2, (p1 > 0 ? (size_t)p1 * 3 : 0), 'f'));
    if (p1 > 0) fake_read("glUniform3fv", p2, (size_t)p1 * 3 * sizeof(float));
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
}

static void fake_glUniform3i(int32_t p0, int32_t p1, int32_t p2, int32_t p3) {
    fake_log("glUniform3i %lld %lld %lld %lld", (long long)p0, (long long)p1, (long long)p2, (long long)p3);
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
    (void)p3;
}

static void fake_glUniform3iv(int32_t p0, int32_t p1, const int32_t * p2) {
    fake_log("glUniform3iv %lld %lld %s", (long long)p0, (long long)p1, fake_values(p2, (p1 > 0 ? (size_t)p1 * 3 : 0), 'i'));
    if (p1 > 0) fake_read("glUniform3iv", p2, (size_t)p1 * 3 * sizeof(int32_t));
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
}

static void fake_glUniform3uiv(int32_t p0, int32_t p1, const uint32_t * p2) {
    fake_log("glUniform3uiv %lld %lld %s", (long long)p0, (long long)p1, fake_values(p2, (p1 > 0 ? (size_t)p1 * 3 : 0), 'u'));
    if (p1 > 0) fake_read("glUniform3uiv", p2, (size_t)p1 * 3 * sizeof(uint32_t));
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
}

static void fake_glUniform4fv(int32_t p0, int32_t p1, const float * p2) {
    fake_log("glUniform4fv %lld %lld %s", (long long)p0, (long long)p1, fake_values(p2, (p1 > 0 ? (size_t)p1 * 4 : 0), 'f'));
    if (p1 > 0) fake_read("glUniform4fv", p2, (size_t)p1 * 4 * sizeof(float));
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
}

static void fake_glUniform4iv(int32_t p0, int32_t p1, const int32_t * p2) {
    fake_log("glUniform4iv %lld %lld %s", (long long)p0, (long long)p1, fake_values(p2, (p1 > 0 ? (size_t)p1 * 4 : 0), 'i'));
    if (p1 > 0) fake_read("glUniform4iv", p2, (size_t)p1 * 4 * sizeof(int32_t));
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
}

static void fake_glUniform4uiv(int32_t p0, int32_t p1, const uint32_t * p2) {
    fake_log("glUniform4uiv %lld %lld %s", (long long)p0, (long long)p1, fake_values(p2, (p1 > 0 ? (size_t)p1 * 4 : 0), 'u'));
    if (p1 > 0) fake_read("glUniform4uiv", p2, (size_t)p1 * 4 * sizeof(uint32_t));
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
}

static void fake_glUniformBlockBinding(uint32_t p0, uint32_t p1, uint32_t p2) {
    fake_log("glUniformBlockBinding %lld %lld %lld", (long long)p0, (long long)p1, (long long)p2);
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
}

static void fake_glUniformMatrix2fv(int32_t p0, int32_t p1, uint8_t p2, const float * p3) {
    fake_log("glUniformMatrix2fv %lld %lld %lld %s", (long long)p0, (long long)p1, (long long)p2, fake_values(p3, (p1 > 0 ? (size_t)p1 * 4 : 0), 'f'));
    if (p1 > 0) fake_read("glUniformMatrix2fv", p3, (size_t)p1 * 4 * sizeof(float));
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
    (void)p3;
}

static void fake_glUniformMatrix2x3fv(int32_t p0, int32_t p1, uint8_t p2, const float * p3) {
    fake_log("glUniformMatrix2x3fv %lld %lld %lld %s", (long long)p0, (long long)p1, (long long)p2, fake_values(p3, (p1 > 0 ? (size_t)p1 * 6 : 0), 'f'));
    if (p1 > 0) fake_read("glUniformMatrix2x3fv", p3, (size_t)p1 * 6 * sizeof(float));
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
    (void)p3;
}

static void fake_glUniformMatrix2x4fv(int32_t p0, int32_t p1, uint8_t p2, const float * p3) {
    fake_log("glUniformMatrix2x4fv %lld %lld %lld %s", (long long)p0, (long long)p1, (long long)p2, fake_values(p3, (p1 > 0 ? (size_t)p1 * 8 : 0), 'f'));
    if (p1 > 0) fake_read("glUniformMatrix2x4fv", p3, (size_t)p1 * 8 * sizeof(float));
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
    (void)p3;
}

static void fake_glUniformMatrix3fv(int32_t p0, int32_t p1, uint8_t p2, const float * p3) {
    fake_log("glUniformMatrix3fv %lld %lld %lld %s", (long long)p0, (long long)p1, (long long)p2, fake_values(p3, (p1 > 0 ? (size_t)p1 * 9 : 0), 'f'));
    if (p1 > 0) fake_read("glUniformMatrix3fv", p3, (size_t)p1 * 9 * sizeof(float));
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
    (void)p3;
}

static void fake_glUniformMatrix3x2fv(int32_t p0, int32_t p1, uint8_t p2, const float * p3) {
    fake_log("glUniformMatrix3x2fv %lld %lld %lld %s", (long long)p0, (long long)p1, (long long)p2, fake_values(p3, (p1 > 0 ? (size_t)p1 * 6 : 0), 'f'));
    if (p1 > 0) fake_read("glUniformMatrix3x2fv", p3, (size_t)p1 * 6 * sizeof(float));
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
    (void)p3;
}

static void fake_glUniformMatrix3x4fv(int32_t p0, int32_t p1, uint8_t p2, const float * p3) {
    fake_log("glUniformMatrix3x4fv %lld %lld %lld %s", (long long)p0, (long long)p1, (long long)p2, fake_values(p3, (p1 > 0 ? (size_t)p1 * 12 : 0), 'f'));
    if (p1 > 0) fake_read("glUniformMatrix3x4fv", p3, (size_t)p1 * 12 * sizeof(float));
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
    (void)p3;
}

static void fake_glUniformMatrix4fv(int32_t p0, int32_t p1, uint8_t p2, const float * p3) {
    fake_log("glUniformMatrix4fv %lld %lld %lld %s", (long long)p0, (long long)p1, (long long)p2, fake_values(p3, (p1 > 0 ? (size_t)p1 * 16 : 0), 'f'));
    if (p1 > 0) fake_read("glUniformMatrix4fv", p3, (size_t)p1 * 16 * sizeof(float));
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
    (void)p3;
}

static void fake_glUniformMatrix4x2fv(int32_t p0, int32_t p1, uint8_t p2, const float * p3) {
    fake_log("glUniformMatrix4x2fv %lld %lld %lld %s", (long long)p0, (long long)p1, (long long)p2, fake_values(p3, (p1 > 0 ? (size_t)p1 * 8 : 0), 'f'));
    if (p1 > 0) fake_read("glUniformMatrix4x2fv", p3, (size_t)p1 * 8 * sizeof(float));
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
    (void)p3;
}

static void fake_glUniformMatrix4x3fv(int32_t p0, int32_t p1, uint8_t p2, const float * p3) {
    fake_log("glUniformMatrix4x3fv %lld %lld %lld %s", (long long)p0, (long long)p1, (long long)p2, fake_values(p3, (p1 > 0 ? (size_t)p1 * 12 : 0), 'f'));
    if (p1 > 0) fake_read("glUniformMatrix4x3fv", p3, (size_t)p1 * 12 * sizeof(float));
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
    (void)p3;
}

static void fake_glUseProgram(uint32_t p0) {
    fake_log("glUseProgram %lld", (long long)p0);
    (void)fake_value();
    (void)p0;
}

static void fake_glVertexAttribDivisor(uint32_t p0, uint32_t p1) {
    fake_log("glVertexAttribDivisor %lld %lld", (long long)p0, (long long)p1);
    (void)fake_value();
    (void)p0;
    (void)p1;
}

static void fake_glViewport(int32_t p0, int32_t p1, int32_t p2, int32_t p3) {
    fake_log("glViewport %lld %lld %lld %lld", (long long)p0, (long long)p1, (long long)p2, (long long)p3);
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
    (void)p3;
}

static void fake_glBlendEquationSeparateiOES(uint32_t p0, uint32_t p1, uint32_t p2) {
    fake_log("glBlendEquationSeparateiOES %lld %lld %lld", (long long)p0, (long long)p1, (long long)p2);
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
}

static void fake_glBlendFuncSeparateiOES(uint32_t p0, uint32_t p1, uint32_t p2, uint32_t p3, uint32_t p4) {
    fake_log("glBlendFuncSeparateiOES %lld %lld %lld %lld %lld", (long long)p0, (long long)p1, (long long)p2, (long long)p3, (long long)p4);
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
    (void)p3;
    (void)p4;
}

static void fake_glClipControlEXT(uint32_t p0, uint32_t p1) {
    fake_log("glClipControlEXT %lld %lld", (long long)p0, (long long)p1);
    (void)fake_value();
    (void)p0;
    (void)p1;
}

static void fake_glColorMaskiOES(uint32_t p0, uint8_t p1, uint8_t p2, uint8_t p3, uint8_t p4) {
    fake_log("glColorMaskiOES %lld %lld %lld %lld %lld", (long long)p0, (long long)p1, (long long)p2, (long long)p3, (long long)p4);
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
    (void)p3;
    (void)p4;
}

static void fake_glDisableiOES(uint32_t p0, uint32_t p1) {
    fake_log("glDisableiOES %lld %lld", (long long)p0, (long long)p1);
    (void)fake_value();
    (void)p0;
    (void)p1;
}

static void fake_glEnableiOES(uint32_t p0, uint32_t p1) {
    fake_log("glEnableiOES %lld %lld", (long long)p0, (long long)p1);
    (void)fake_value();
    (void)p0;
    (void)p1;
}

static void fake_glGetQueryObjectui64vEXT(uint32_t p0, uint32_t p1, uint64_t * p2) {
    fake_log("glGetQueryObjectui64vEXT %lld %lld %s", (long long)p0, (long long)p1, FAKE_ADDRESS(p2));
    fake_write("glGetQueryObjectui64vEXT", p2, 16 * sizeof(uint64_t));
    fake_log("  wrote 2 %s", fake_values(p2, 16, 'U'));
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
}

static void fake_glGetQueryivEXT(uint32_t p0, uint32_t p1, int32_t * p2) {
    fake_log("glGetQueryivEXT %lld %lld %s", (long long)p0, (long long)p1, FAKE_ADDRESS(p2));
    fake_write("glGetQueryivEXT", p2, 16 * sizeof(int32_t));
    fake_log("  wrote 2 %s", fake_values(p2, 16, 'i'));
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
}

static void fake_glPolygonModeANGLE(uint32_t p0, uint32_t p1) {
    fake_log("glPolygonModeANGLE %lld %lld", (long long)p0, (long long)p1);
    (void)fake_value();
    (void)p0;
    (void)p1;
}

static void fake_glPolygonOffsetClampEXT(float p0, float p1, float p2) {
    fake_log("glPolygonOffsetClampEXT %.9g %.9g %.9g", (double)p0, (double)p1, (double)p2);
    (void)fake_value();
    (void)p0;
    (void)p1;
    (void)p2;
}

static void fake_glProvokingVertexANGLE(uint32_t p0) {
    fake_log("glProvokingVertexANGLE %lld", (long long)p0);
    (void)fake_value();
    (void)p0;
}

static void fake_glQueryCounterEXT(uint32_t p0, uint32_t p1) {
    fake_log("glQueryCounterEXT %lld %lld", (long long)p0, (long long)p1);
    (void)fake_value();
    (void)p0;
    (void)p1;
}

void *fake_gl_lookup(const char *name) {
    static const struct {
        const char *name;
        void *address;
    } table[] = {
        {"glActiveTexture", (void *)fake_glActiveTexture},
        {"glAttachShader", (void *)fake_glAttachShader},
        {"glBeginQuery", (void *)fake_glBeginQuery},
        {"glBeginTransformFeedback", (void *)fake_glBeginTransformFeedback},
        {"glBindBuffer", (void *)fake_glBindBuffer},
        {"glBindBufferBase", (void *)fake_glBindBufferBase},
        {"glBindBufferRange", (void *)fake_glBindBufferRange},
        {"glBindFramebuffer", (void *)fake_glBindFramebuffer},
        {"glBindRenderbuffer", (void *)fake_glBindRenderbuffer},
        {"glBindSampler", (void *)fake_glBindSampler},
        {"glBindTexture", (void *)fake_glBindTexture},
        {"glBindVertexArray", (void *)fake_glBindVertexArray},
        {"glBlendColor", (void *)fake_glBlendColor},
        {"glBlendEquationSeparate", (void *)fake_glBlendEquationSeparate},
        {"glBlendEquationSeparateiOES", (void *)fake_glBlendEquationSeparateiOES},
        {"glBlendFuncSeparate", (void *)fake_glBlendFuncSeparate},
        {"glBlendFuncSeparateiOES", (void *)fake_glBlendFuncSeparateiOES},
        {"glBlitFramebuffer", (void *)fake_glBlitFramebuffer},
        {"glBufferData", (void *)fake_glBufferData},
        {"glBufferSubData", (void *)fake_glBufferSubData},
        {"glCheckFramebufferStatus", (void *)fake_glCheckFramebufferStatus},
        {"glClearBufferfi", (void *)fake_glClearBufferfi},
        {"glClearBufferfv", (void *)fake_glClearBufferfv},
        {"glClearBufferiv", (void *)fake_glClearBufferiv},
        {"glClearBufferuiv", (void *)fake_glClearBufferuiv},
        {"glClientWaitSync", (void *)fake_glClientWaitSync},
        {"glClipControlEXT", (void *)fake_glClipControlEXT},
        {"glColorMask", (void *)fake_glColorMask},
        {"glColorMaskiOES", (void *)fake_glColorMaskiOES},
        {"glCompileShader", (void *)fake_glCompileShader},
        {"glCompressedTexSubImage2D", (void *)fake_glCompressedTexSubImage2D},
        {"glCompressedTexSubImage3D", (void *)fake_glCompressedTexSubImage3D},
        {"glCopyBufferSubData", (void *)fake_glCopyBufferSubData},
        {"glCopyTexSubImage2D", (void *)fake_glCopyTexSubImage2D},
        {"glCopyTexSubImage3D", (void *)fake_glCopyTexSubImage3D},
        {"glCreateProgram", (void *)fake_glCreateProgram},
        {"glCreateShader", (void *)fake_glCreateShader},
        {"glCullFace", (void *)fake_glCullFace},
        {"glDeleteBuffers", (void *)fake_glDeleteBuffers},
        {"glDeleteFramebuffers", (void *)fake_glDeleteFramebuffers},
        {"glDeleteProgram", (void *)fake_glDeleteProgram},
        {"glDeleteQueries", (void *)fake_glDeleteQueries},
        {"glDeleteRenderbuffers", (void *)fake_glDeleteRenderbuffers},
        {"glDeleteSamplers", (void *)fake_glDeleteSamplers},
        {"glDeleteShader", (void *)fake_glDeleteShader},
        {"glDeleteSync", (void *)fake_glDeleteSync},
        {"glDeleteTextures", (void *)fake_glDeleteTextures},
        {"glDeleteVertexArrays", (void *)fake_glDeleteVertexArrays},
        {"glDepthFunc", (void *)fake_glDepthFunc},
        {"glDepthMask", (void *)fake_glDepthMask},
        {"glDepthRangef", (void *)fake_glDepthRangef},
        {"glDetachShader", (void *)fake_glDetachShader},
        {"glDisable", (void *)fake_glDisable},
        {"glDisableVertexAttribArray", (void *)fake_glDisableVertexAttribArray},
        {"glDisableiOES", (void *)fake_glDisableiOES},
        {"glDrawArrays", (void *)fake_glDrawArrays},
        {"glDrawArraysInstanced", (void *)fake_glDrawArraysInstanced},
        {"glDrawBuffers", (void *)fake_glDrawBuffers},
        {"glDrawElementsInstanced", (void *)fake_glDrawElementsInstanced},
        {"glEnable", (void *)fake_glEnable},
        {"glEnableVertexAttribArray", (void *)fake_glEnableVertexAttribArray},
        {"glEnableiOES", (void *)fake_glEnableiOES},
        {"glEndQuery", (void *)fake_glEndQuery},
        {"glEndTransformFeedback", (void *)fake_glEndTransformFeedback},
        {"glFenceSync", (void *)fake_glFenceSync},
        {"glFinish", (void *)fake_glFinish},
        {"glFlush", (void *)fake_glFlush},
        {"glFramebufferRenderbuffer", (void *)fake_glFramebufferRenderbuffer},
        {"glFramebufferTexture2D", (void *)fake_glFramebufferTexture2D},
        {"glFramebufferTextureLayer", (void *)fake_glFramebufferTextureLayer},
        {"glFrontFace", (void *)fake_glFrontFace},
        {"glGenBuffers", (void *)fake_glGenBuffers},
        {"glGenFramebuffers", (void *)fake_glGenFramebuffers},
        {"glGenQueries", (void *)fake_glGenQueries},
        {"glGenRenderbuffers", (void *)fake_glGenRenderbuffers},
        {"glGenSamplers", (void *)fake_glGenSamplers},
        {"glGenTextures", (void *)fake_glGenTextures},
        {"glGenVertexArrays", (void *)fake_glGenVertexArrays},
        {"glGenerateMipmap", (void *)fake_glGenerateMipmap},
        {"glGetBooleanv", (void *)fake_glGetBooleanv},
        {"glGetError", (void *)fake_glGetError},
        {"glGetFloatv", (void *)fake_glGetFloatv},
        {"glGetInteger64v", (void *)fake_glGetInteger64v},
        {"glGetIntegerv", (void *)fake_glGetIntegerv},
        {"glGetInternalformativ", (void *)fake_glGetInternalformativ},
        {"glGetProgramInfoLog", (void *)fake_glGetProgramInfoLog},
        {"glGetProgramiv", (void *)fake_glGetProgramiv},
        {"glGetQueryObjectui64vEXT", (void *)fake_glGetQueryObjectui64vEXT},
        {"glGetQueryObjectuiv", (void *)fake_glGetQueryObjectuiv},
        {"glGetQueryivEXT", (void *)fake_glGetQueryivEXT},
        {"glGetShaderInfoLog", (void *)fake_glGetShaderInfoLog},
        {"glGetShaderiv", (void *)fake_glGetShaderiv},
        {"glGetString", (void *)fake_glGetString},
        {"glGetStringi", (void *)fake_glGetStringi},
        {"glGetSynciv", (void *)fake_glGetSynciv},
        {"glGetUniformBlockIndex", (void *)fake_glGetUniformBlockIndex},
        {"glGetUniformLocation", (void *)fake_glGetUniformLocation},
        {"glGetVertexAttribiv", (void *)fake_glGetVertexAttribiv},
        {"glInvalidateFramebuffer", (void *)fake_glInvalidateFramebuffer},
        {"glLinkProgram", (void *)fake_glLinkProgram},
        {"glMapBufferRange", (void *)fake_glMapBufferRange},
        {"glPixelStorei", (void *)fake_glPixelStorei},
        {"glPolygonModeANGLE", (void *)fake_glPolygonModeANGLE},
        {"glPolygonOffset", (void *)fake_glPolygonOffset},
        {"glPolygonOffsetClampEXT", (void *)fake_glPolygonOffsetClampEXT},
        {"glProvokingVertexANGLE", (void *)fake_glProvokingVertexANGLE},
        {"glQueryCounterEXT", (void *)fake_glQueryCounterEXT},
        {"glReadPixels", (void *)fake_glReadPixels},
        {"glRenderbufferStorage", (void *)fake_glRenderbufferStorage},
        {"glRenderbufferStorageMultisample", (void *)fake_glRenderbufferStorageMultisample},
        {"glSamplerParameterf", (void *)fake_glSamplerParameterf},
        {"glSamplerParameteri", (void *)fake_glSamplerParameteri},
        {"glScissor", (void *)fake_glScissor},
        {"glShaderSource", (void *)fake_glShaderSource},
        {"glStencilFunc", (void *)fake_glStencilFunc},
        {"glStencilFuncSeparate", (void *)fake_glStencilFuncSeparate},
        {"glStencilMask", (void *)fake_glStencilMask},
        {"glStencilOp", (void *)fake_glStencilOp},
        {"glStencilOpSeparate", (void *)fake_glStencilOpSeparate},
        {"glTexParameteri", (void *)fake_glTexParameteri},
        {"glTexStorage2D", (void *)fake_glTexStorage2D},
        {"glTexStorage3D", (void *)fake_glTexStorage3D},
        {"glTexSubImage2D", (void *)fake_glTexSubImage2D},
        {"glTexSubImage3D", (void *)fake_glTexSubImage3D},
        {"glTransformFeedbackVaryings", (void *)fake_glTransformFeedbackVaryings},
        {"glUniform1f", (void *)fake_glUniform1f},
        {"glUniform1fv", (void *)fake_glUniform1fv},
        {"glUniform1i", (void *)fake_glUniform1i},
        {"glUniform1iv", (void *)fake_glUniform1iv},
        {"glUniform1ui", (void *)fake_glUniform1ui},
        {"glUniform1uiv", (void *)fake_glUniform1uiv},
        {"glUniform2f", (void *)fake_glUniform2f},
        {"glUniform2fv", (void *)fake_glUniform2fv},
        {"glUniform2iv", (void *)fake_glUniform2iv},
        {"glUniform2uiv", (void *)fake_glUniform2uiv},
        {"glUniform3fv", (void *)fake_glUniform3fv},
        {"glUniform3i", (void *)fake_glUniform3i},
        {"glUniform3iv", (void *)fake_glUniform3iv},
        {"glUniform3uiv", (void *)fake_glUniform3uiv},
        {"glUniform4fv", (void *)fake_glUniform4fv},
        {"glUniform4iv", (void *)fake_glUniform4iv},
        {"glUniform4uiv", (void *)fake_glUniform4uiv},
        {"glUniformBlockBinding", (void *)fake_glUniformBlockBinding},
        {"glUniformMatrix2fv", (void *)fake_glUniformMatrix2fv},
        {"glUniformMatrix2x3fv", (void *)fake_glUniformMatrix2x3fv},
        {"glUniformMatrix2x4fv", (void *)fake_glUniformMatrix2x4fv},
        {"glUniformMatrix3fv", (void *)fake_glUniformMatrix3fv},
        {"glUniformMatrix3x2fv", (void *)fake_glUniformMatrix3x2fv},
        {"glUniformMatrix3x4fv", (void *)fake_glUniformMatrix3x4fv},
        {"glUniformMatrix4fv", (void *)fake_glUniformMatrix4fv},
        {"glUniformMatrix4x2fv", (void *)fake_glUniformMatrix4x2fv},
        {"glUniformMatrix4x3fv", (void *)fake_glUniformMatrix4x3fv},
        {"glUnmapBuffer", (void *)fake_glUnmapBuffer},
        {"glUseProgram", (void *)fake_glUseProgram},
        {"glVertexAttribDivisor", (void *)fake_glVertexAttribDivisor},
        {"glVertexAttribIPointer", (void *)fake_glVertexAttribIPointer},
        {"glVertexAttribPointer", (void *)fake_glVertexAttribPointer},
        {"glViewport", (void *)fake_glViewport},
    };
    for (size_t at = 0; at < sizeof table / sizeof table[0]; at++)
        if (strcmp(table[at].name, name) == 0) return table[at].address;
    return NULL;
}
