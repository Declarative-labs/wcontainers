// Counted loops over memory, alone and inside each other, whose accesses move by a fixed step: some stay
// inside the blob, the last runs onto the page after it, which allows nothing, and faults there. Each sum
// is printed. Output and exit status are checked against real ARM64 Linux.

typedef unsigned long u64;
typedef long i64;
typedef unsigned int u32;
typedef unsigned char u8;

static i64 syscall3(i64 number, i64 a, i64 b, i64 c) {
    register i64 x8 __asm__("x8") = number;
    register i64 x0 __asm__("x0") = a;
    register i64 x1 __asm__("x1") = b;
    register i64 x2 __asm__("x2") = c;
    __asm__ volatile("svc #0" : "+r"(x0) : "r"(x8), "r"(x1), "r"(x2) : "memory");
    return x0;
}

// The first byte after the blob, from the linker script.
extern char __blob_end[];

static void result(u64 value) {
    char line[24];
    int count = 0;
    char digits[20];
    do {
        digits[count++] = '0' + value % 10;
        value /= 10;
    } while (value);
    int at = 0;
    while (count) line[at++] = digits[--count];
    line[at++] = '\n';
    syscall3(64, 1, (i64)line, at);
}

static u8 bytes[3 * 4096];
static u64 words[64 * 64];

__attribute__((noinline)) u64 rows(u32 n) {
    u64 sum = 0;
    for (u32 r = 0; r < n; r++) {
        sum += bytes[r * 1000];
        for (u32 c = 0; c < 1000; c++) bytes[r * 1000 + c] += (u8)(r ^ c);
    }
    for (u32 i = 0; i < sizeof bytes; i += 7) sum += bytes[i];
    return sum;
}

__attribute__((noinline)) u64 columns(u32 n) {
    u64 sum = 0;
    for (u32 i = 0; i < n * n; i++) words[i] = i * 2654435761u;
    for (u32 j = 0; j < n; j++)
        for (u32 i = n; i > 0; i--) sum += words[(i - 1) * n + j] ^ j;
    return sum;
}

__attribute__((noinline)) u64 backwards(i64 from) {
    u64 sum = 0;
    for (i64 i = from; i >= 0; i -= 3) sum = sum * 31 + bytes[i];
    return sum;
}

// Reads up to `past` bytes beyond the page-aligned end of the blob, `step` bytes at a time.
__attribute__((noinline)) u64 onto_the_edge(const u8 *edge, i64 past, i64 step) {
    u64 sum = 0;
    for (i64 i = -4096; i < past; i += step) {
        sum += edge[i];
        for (u32 k = 0; k < 4; k++) bytes[k] += (u8)i;
    }
    return sum;
}

__attribute__((section(".text._start"), noreturn)) void _start(void) {
    const u8 *edge = (const u8 *)(((u64)__blob_end + 4095) & ~4095ul);
    result(rows(12));
    result(columns(64));
    result(backwards(sizeof bytes - 1));
    result(onto_the_edge(edge, 0, 8));
    result(onto_the_edge(edge, 0, 4096));
    result(onto_the_edge(edge, 64, 8));
    syscall3(94, 5, 0, 0);
    for (;;) {
    }
}
