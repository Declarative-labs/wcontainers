// rep string instructions: copies and fills of every element size, one longer than a bulk pass, copies
// backwards with the direction flag set, a copy onto a destination inside its source, a fill of a value that
// is not one byte repeated, repe cmps and repne scas with their flags read after them, a count of zero that
// keeps the flags of a compare before it, and rep lods. Prints a sum of everything and exits 0.

typedef unsigned long u64;

#include "syscall.h"

#define BIG (3 << 20)
#define SMALL 4096

static unsigned char a[BIG + SMALL], b[BIG + SMALL];
static u64 sum;

static void mix(u64 v) {
    sum = (sum ^ v) * 0x100000001b3ul;
}

static void mix_bytes(const unsigned char *at, u64 n) {
    for (u64 i = 0; i < n; i++) mix(at[i] + i);
}

#define MOVS(suffix)                                                                                     \
    static void movs##suffix(void *to, const void *from, u64 n) {                                       \
        __asm__ volatile("rep movs" #suffix : "+D"(to), "+S"(from), "+c"(n) : : "memory");              \
        mix((u64)to ^ (u64)from ^ n);                                                                    \
    }
MOVS(b)
MOVS(w)
MOVS(l)
MOVS(q)

#define STOS(suffix)                                                                                     \
    static void stos##suffix(void *to, u64 value, u64 n) {                                              \
        __asm__ volatile("rep stos" #suffix : "+D"(to), "+c"(n) : "a"(value) : "memory");               \
        mix((u64)to ^ n);                                                                                \
    }
STOS(b)
STOS(w)
STOS(l)
STOS(q)

static void backwards(void *to_last, const void *from_last, u64 n) {
    __asm__ volatile("std\n rep movsb\n cld" : "+D"(to_last), "+S"(from_last), "+c"(n) : : "memory");
    mix((u64)to_last ^ (u64)from_last ^ n);
}

// repe cmpsb: the flags and registers after it.
static void compare(const void *x, const void *y, u64 n) {
    unsigned char z, c, s;
    __asm__ volatile("repe cmpsb\n setz %3\n setc %4\n sets %5"
                     : "+S"(x), "+D"(y), "+c"(n), "=r"(z), "=r"(c), "=r"(s)
                     :
                     : "memory", "cc");
    mix((u64)x ^ (u64)y ^ n ^ z ^ (u64)c << 8 ^ (u64)s << 16);
}

// repne scasb for a byte: where it stopped and whether it found it.
static void scan(const void *at, unsigned char byte, u64 n) {
    unsigned char z;
    __asm__ volatile("repne scasb\n setz %2" : "+D"(at), "+c"(n), "=r"(z) : "a"(byte) : "memory", "cc");
    mix((u64)at ^ n ^ z);
}

// A count of zero: the flags are those of the compare before it.
static void none(const void *x, const void *y, u64 left, u64 right) {
    unsigned char c, z;
    u64 n = 0;
    __asm__ volatile("cmp %6, %5\n repe cmpsb\n setc %3\n setz %4"
                     : "+S"(x), "+D"(y), "+c"(n), "=r"(c), "=r"(z)
                     : "r"(left), "r"(right)
                     : "memory", "cc");
    mix((u64)c << 1 ^ z ^ (u64)x ^ (u64)y);
}

static u64 load(const void *at, u64 n) {
    u64 value = 0;
    __asm__ volatile("rep lodsq" : "+S"(at), "+c"(n), "+a"(value) : : "memory");
    return value ^ (u64)at;
}

void _start(void) {
    for (u64 i = 0; i < BIG; i++) a[i] = (unsigned char)(i * 7 + (i >> 9));
    movsb(b, a, BIG);
    mix_bytes(b + BIG - SMALL, SMALL);
    movsq(b + 8, a + 16, BIG / 8 - 4);
    mix_bytes(b, SMALL);
    movsw(b + 3, a + 5, 1001);
    movsl(b + 1, a + 9, 777);
    mix_bytes(b, SMALL);
    stosb(b, 0, BIG);
    mix_bytes(b + 12345, SMALL);
    stosw(b + 1, 0x5a5a, 999);
    stosl(b + 2, 0x01020304, 513);
    stosq(b + 7, 0xfedcba9876543210ul, 300);
    stosq(b + SMALL, 0, (BIG - SMALL) / 8);
    mix_bytes(b, 2 * SMALL);
    backwards(b + SMALL - 1, a + SMALL - 1, SMALL);
    mix_bytes(b, SMALL);
    // A forward copy onto its own source a few bytes up repeats the first bytes.
    movsb(a + 3, a, SMALL);
    mix_bytes(a, 2 * SMALL);
    movsq(a + 8, a, SMALL / 8);
    mix_bytes(a, 2 * SMALL);
    movsb(b, a, SMALL);
    compare(a, b, SMALL);
    b[1000] ^= 1;
    compare(a, b, SMALL);
    compare(b, a, SMALL);
    b[SMALL - 1] = 0x7f;
    scan(b, 0x7f, SMALL);
    scan(b, 0x7f, 10);
    none(a, b, 3, 9);
    none(a, b, 9, 9);
    mix(load(a, 100));
    char text[17];
    u64 s = sum;
    for (int n = 15; n >= 0; n--, s >>= 4) text[n] = "0123456789abcdef"[s & 15];
    text[16] = '\n';
    syscall3(SYS_write, 1, (long)text, 17);
    syscall3(SYS_exit, 0, 0, 0);
    __builtin_unreachable();
}
