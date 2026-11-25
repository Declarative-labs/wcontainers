// Tables in zeroed memory only, with no initialised data: a sieve, an open addressing hash table of 16 bit
// keys, byte histograms, and a dense switch that compilers turn into a jump table. Output and exit status are
// checked against real Linux.

typedef unsigned long u64;
typedef long i64;
typedef unsigned int u32;
typedef unsigned short u16;
typedef unsigned char u8;

#include "syscall.h"

static char out[2048];
static u32 used;
static u8 composite[20000];
static u16 keys[1024];
static u32 counts[1024];
static u32 histogram[256];
static u64 seed;

static void put(char c) {
    if (used == sizeof out) {
        syscall3(SYS_write, 1, (i64)out, used);
        used = 0;
    }
    out[used++] = c;
}

static void text(const char *s) {
    while (*s) put(*s++);
}

static void number(u64 value) {
    char digits[24];
    int n = 0;
    do {
        digits[n++] = (char)('0' + value % 10);
        value /= 10;
    } while (value);
    while (n) put(digits[--n]);
}

static void line(const char *label, u64 value) {
    text(label);
    put(' ');
    number(value);
    put('\n');
}

static u64 next(void) {
    seed = seed * 6364136223846793005ul + 1442695040888963407ul;
    return seed >> 33;
}

static u32 sieve(void) {
    u32 primes = 0;
    for (u32 i = 2; i < sizeof composite; i++) {
        if (composite[i]) continue;
        primes++;
        for (u32 j = i * 2; j < sizeof composite; j += i) composite[j] = 1;
    }
    return primes;
}

// Count each key; the table never fills, since there are fewer distinct keys than slots.
static void insert(u16 key) {
    u32 slot = (key * 40503u) >> 6 & 1023;
    while (counts[slot] && keys[slot] != key) slot = (slot + 1) & 1023;
    keys[slot] = key;
    counts[slot]++;
}

static u32 lookup(u16 key) {
    u32 slot = (key * 40503u) >> 6 & 1023;
    while (counts[slot]) {
        if (keys[slot] == key) return counts[slot];
        slot = (slot + 1) & 1023;
    }
    return 0;
}

static __attribute__((noinline)) u64 step(u32 op, u64 acc, u64 x) {
    switch (op) {
    case 0: return acc + x;
    case 1: return acc - x;
    case 2: return acc ^ x;
    case 3: return acc * (x | 1);
    case 4: return acc >> (x & 63);
    case 5: return acc << (x & 63);
    case 6: return acc | x;
    case 7: return acc & ~x;
    case 8: return acc / (x | 1);
    case 9: return acc % (x | 1);
    case 10: return (acc << 7) | (acc >> 57);
    case 11: return acc + (x << 32);
    default: return ~acc;
    }
}

void _start(void) {
    seed = 7;
    u32 primes = sieve();
    line("primes below 20000:", primes);

    for (u32 i = 0; i < 5000; i++) insert((u16)(next() % 700));
    u32 distinct = 0, largest = 0;
    for (u32 i = 0; i < 1024; i++) {
        if (counts[i]) distinct++;
        if (counts[i] > largest) largest = counts[i];
    }
    line("distinct keys:", distinct);
    line("largest count:", largest);
    line("count of 123:", lookup(123));
    line("count of 701:", lookup(701));

    for (u32 i = 0; i < 20000; i++) histogram[(u8)(next() >> 3)]++;
    u64 weighted = 0;
    for (u32 i = 0; i < 256; i++) weighted += (u64)histogram[i] * i;
    line("weighted histogram:", weighted);

    u64 acc = 1;
    for (u32 i = 0; i < 1000; i++) acc = step((u32)(next() % 13), acc, next());
    line("switch result:", acc);

    syscall3(SYS_write, 1, (i64)out, used);
    syscall3(SYS_exit, (i64)(acc & 0x7f), 0, 0);
    for (;;) {}
}
