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

/* EXTENSIONS */

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
