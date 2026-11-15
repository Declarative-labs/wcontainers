// Integer work a compiler turns into many instruction forms: arithmetic, division, shifts, bit tricks,
// sorting through a function pointer, recursion and printing numbers. Output and exit status are checked
// against real ARM64 Linux.

typedef unsigned long u64;
typedef long i64;
typedef unsigned int u32;
typedef int i32;

static i64 syscall3(i64 number, i64 a, i64 b, i64 c) {
    register i64 x8 __asm__("x8") = number;
    register i64 x0 __asm__("x0") = a;
    register i64 x1 __asm__("x1") = b;
    register i64 x2 __asm__("x2") = c;
    __asm__ volatile("svc #0" : "+r"(x0) : "r"(x8), "r"(x1), "r"(x2) : "memory");
    return x0;
}

static char buffer[4096];
static u32 used;

static void flush(void) {
    syscall3(64, 1, (i64)buffer, used);
    used = 0;
}

static void put(char c) {
    if (used == sizeof buffer) flush();
    buffer[used++] = c;
}

static void text(const char *s) {
    while (*s) put(*s++);
}

static void number(i64 value) {
    char digits[24];
    int n = 0;
    u64 magnitude = value < 0 ? -(u64)value : (u64)value;
    do {
        digits[n++] = (char)('0' + magnitude % 10);
        magnitude /= 10;
    } while (magnitude);
    if (value < 0) put('-');
    while (n) put(digits[--n]);
}

static void hex(u64 value) {
    text("0x");
    for (int shift = 60; shift >= 0; shift -= 4) put("0123456789abcdef"[(value >> shift) & 15]);
}

static void line(const char *label, i64 value) {
    text(label);
    text(" ");
    number(value);
    put('\n');
}

static u64 state = 0x243f6a8885a308d3;

static u64 next(void) {
    state ^= state << 13;
    state ^= state >> 7;
    state ^= state << 17;
    return state;
}

static int compare_signed(const void *a, const void *b) {
    i64 x = *(const i64 *)a, y = *(const i64 *)b;
    return (x > y) - (x < y);
}

static int compare_unsigned(const void *a, const void *b) {
    u64 x = *(const u64 *)a, y = *(const u64 *)b;
    return (x > y) - (x < y);
}

static void sort(void *base, u32 count, u32 size, int (*compare)(const void *, const void *)) {
    char *items = base;
    for (u32 i = 1; i < count; i++) {
        for (u32 j = i; j > 0 && compare(items + (j - 1) * size, items + j * size) > 0; j--) {
            for (u32 k = 0; k < size; k++) {
                char t = items[(j - 1) * size + k];
                items[(j - 1) * size + k] = items[j * size + k];
                items[j * size + k] = t;
            }
        }
    }
}

// Counted by hand: compilers turn the builtin into vector instructions.
static i64 ones(u64 v) {
    i64 n = 0;
    for (; v; v &= v - 1) n++;
    return n;
}

static u64 fibonacci(u32 n) {
    return n < 2 ? n : fibonacci(n - 1) + fibonacci(n - 2);
}

static u32 primes_below(u32 limit) {
    static unsigned char composite[20000];
    u32 count = 0;
    for (u32 i = 2; i < limit; i++) {
        if (composite[i]) continue;
        count++;
        for (u32 j = i * i; j < limit; j += i) composite[j] = 1;
    }
    return count;
}

static i64 values[64];

int main(void) {
    text("integers\n");
    line("fibonacci(24)", (i64)fibonacci(24));
    line("primes below 20000", primes_below(20000));

    for (int i = 0; i < 64; i++) values[i] = (i64)next();
    sort(values, 64, sizeof values[0], compare_signed);
    u64 mix = 0;
    for (int i = 0; i < 64; i++) mix = mix * 31 + (u64)values[i];
    text("sorted signed ");
    hex(mix);
    put('\n');
    sort(values, 64, sizeof values[0], compare_unsigned);
    mix = 0;
    for (int i = 0; i < 64; i++) mix = (mix ^ (u64)values[i]) * 0x100000001b3;
    text("sorted unsigned ");
    hex(mix);
    put('\n');

    i64 quotients = 0;
    for (int i = 0; i < 2000; i++) {
        i64 a = (i64)next(), b = (i64)(next() >> (i % 60));
        u32 c = (u32)next(), d = (u32)next() >> (i % 31);
        if (b) quotients += a / b + a % b;
        if (d) quotients ^= (i64)(c / d) - (i64)(c % d);
        quotients += (i64)__builtin_clzll((u64)a | 1) + ones((u64)b);
        quotients ^= (i64)__builtin_bswap64((u64)a) >> (i % 64);
        quotients += (i32)((i32)c >> (i % 32)) * (i64)(short)d;
    }
    line("division and bits", quotients);

    u64 wide = 0;
    for (int i = 0; i < 500; i++) {
        unsigned __int128 p = (unsigned __int128)next() * next();
        __int128 q = (__int128)(i64)next() * (i64)next();
        wide += (u64)(p >> 64) ^ (u64)(q >> 64);
    }
    text("high halves ");
    hex(wide);
    put('\n');

    flush();
    return (int)(mix & 0x7f);
}

__attribute__((section(".text._start"), noreturn)) void _start(void) {
    int status = main();
    syscall3(94, status, 0, 0);
    __builtin_unreachable();
}
