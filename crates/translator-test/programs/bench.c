// The benchmark set: the toy translator's four workloads (calls, a sieve, a hash, a sort through a comparison
// callback), and more of the same kind: a byte code interpreter built on a switch, a matrix product, a walk
// along a linked list and a byte search. WORKLOAD picks one (0 runs them all). SCALE 0 is small enough for
// look-ahead none in tests; SCALE 1 is the toy's size, for timing.

typedef unsigned long u64;
typedef long i64;
typedef unsigned int u32;
typedef unsigned char u8;

#ifndef WORKLOAD
#define WORKLOAD 0
#endif
#ifndef SCALE
#define SCALE 0
#endif
#define BIG(small, big) (SCALE ? (big) : (small))

#include "syscall.h"

static char out[1024];
static u32 used;

static void put(char c) { out[used++] = c; }

static void text(const char *s) {
    while (*s) put(*s++);
}

static void line(const char *label, u64 value) {
    char digits[24];
    int n = 0;
    text(label);
    put(' ');
    do {
        digits[n++] = (char)('0' + value % 10);
        value /= 10;
    } while (value);
    while (n) put(digits[--n]);
    put('\n');
    syscall3(SYS_write, 1, (i64)out, used);
    used = 0;
}

// The toy's workloads, unchanged.

__attribute__((noinline)) u32 fib(u32 n) { return n < 2 ? n : fib(n - 1) + fib(n - 2); }

__attribute__((noinline)) u32 sieve(u8 *flags, u32 n) {
    u32 count = 0;
    for (u32 i = 0; i < n; i++) flags[i] = 1;
    for (u32 i = 2; i < n; i++) {
        if (flags[i]) {
            count++;
            for (u32 j = i + i; j < n; j += i) flags[j] = 0;
        }
    }
    return count;
}

__attribute__((noinline)) u64 hash(const u8 *p, u32 n, u32 rounds) {
    u64 h = 1469598103934665603ULL;
    for (u32 r = 0; r < rounds; r++)
        for (u32 i = 0; i < n; i++) {
            h ^= p[i];
            h *= 1099511628211ULL;
            h ^= h >> 29;
        }
    return h;
}

typedef int (*cmp_fn)(const u32 *, const u32 *);
__attribute__((noinline)) int cmp_u32(const u32 *a, const u32 *b) { return *a < *b ? -1 : *a > *b; }
__attribute__((noinline)) void quick(u32 *v, int lo, int hi, cmp_fn cmp) {
    while (lo < hi) {
        u32 pivot = v[(lo + hi) / 2];
        int i = lo, j = hi;
        while (i <= j) {
            while (cmp(&v[i], &pivot) < 0) i++;
            while (cmp(&v[j], &pivot) > 0) j--;
            if (i <= j) {
                u32 t = v[i];
                v[i] = v[j];
                v[j] = t;
                i++;
                j--;
            }
        }
        if (lo < j) quick(v, lo, j, cmp);
        lo = i;
    }
}
__attribute__((noinline)) u32 sort_run(u32 *v, u32 n, u32 rounds) {
    u32 acc = 0;
    for (u32 r = 0; r < rounds; r++) {
        u32 x = 12345 + r;
        for (u32 i = 0; i < n; i++) {
            x = x * 1664525u + 1013904223u;
            v[i] = x >> 8;
        }
        quick(v, 0, (int)n - 1, cmp_u32);
        acc += v[n / 2] ^ v[0] ^ v[n - 1];
    }
    return acc;
}

// More of our own.

// A stack machine whose dispatch is a dense switch.
enum { PUSH, ADD, SUB, MUL, DUP, SWAP, JNZ, DEC, XOR, SHR, POP, HALT };
__attribute__((noinline)) u64 interpret(const u8 *code, u64 start) {
    u64 stack[16];
    int sp = 0;
    u32 pc = 0;
    stack[sp++] = start;
    for (;;) {
        u8 op = code[pc++];
        switch (op) {
        case PUSH: stack[sp++] = code[pc++]; break;
        case ADD: sp--; stack[sp - 1] += stack[sp]; break;
        case SUB: sp--; stack[sp - 1] -= stack[sp]; break;
        case MUL: sp--; stack[sp - 1] *= stack[sp]; break;
        case DUP: stack[sp] = stack[sp - 1]; sp++; break;
        case SWAP: { u64 t = stack[sp - 1]; stack[sp - 1] = stack[sp - 2]; stack[sp - 2] = t; break; }
        case JNZ: { u8 to = code[pc++]; if (stack[sp - 1]) pc = to; break; }
        case DEC: stack[sp - 1]--; break;
        case XOR: sp--; stack[sp - 1] ^= stack[sp]; break;
        case SHR: stack[sp - 1] >>= 3; break;
        case POP: sp--; break;
        case HALT: return stack[sp - 1] ^ (u64)sp;
        }
    }
}
// Starting from n: acc = 0; do { acc = ((acc * 31) ^ (acc >> 3)) + 1; } while (--n); the result is acc.
static const u8 program[] = {
    PUSH, 0, SWAP,                  // acc n
    SWAP, DUP, PUSH, 31, MUL,       // 3: n acc acc*31
    SWAP, SHR, XOR, PUSH, 1, ADD,   // n acc'
    SWAP, DEC, JNZ, 3,              // acc' n-1, again while n-1
    POP, HALT,
};
__attribute__((noinline)) u64 interpret_run(u32 rounds, u32 n) {
    u64 acc = 0;
    for (u32 r = 0; r < rounds; r++) acc += interpret(program, n + r);
    return acc;
}

__attribute__((noinline)) u64 matrix(u64 *a, u64 *b, u64 *c, u32 n, u32 rounds) {
    for (u32 i = 0; i < n * n; i++) {
        a[i] = i * 2654435761u;
        b[i] = i ^ (i >> 3);
    }
    u64 sum = 0;
    for (u32 r = 0; r < rounds; r++) {
        for (u32 i = 0; i < n; i++)
            for (u32 j = 0; j < n; j++) {
                u64 s = r;
                for (u32 k = 0; k < n; k++) s += a[i * n + k] * b[k * n + j];
                c[i * n + j] = s;
            }
        sum += c[(r * 7) % (n * n)];
    }
    return sum;
}

struct node { struct node *next; u64 value; };
__attribute__((noinline)) u64 walk(struct node *nodes, u32 n, u32 rounds) {
    // Link every node once, in a scattered order: 7919 has no factor in common with the sizes used.
    for (u32 i = 0; i < n; i++) nodes[i].value = i;
    struct node *head = &nodes[0];
    struct node *at = head;
    for (u32 k = 1; k < n; k++) {
        at->next = &nodes[(u64)k * 7919 % n];
        at = at->next;
    }
    at->next = 0;
    u64 sum = 0;
    for (u32 r = 0; r < rounds; r++)
        for (struct node *p = head; p; p = p->next) sum = sum * 3 + p->value;
    return sum;
}

__attribute__((noinline)) u64 search(u8 *text_bytes, u32 n, u32 rounds) {
    for (u32 i = 0; i < n; i++) text_bytes[i] = (u8)("abcab"[i % 5] + (i % 97 == 0));
    u64 found = 0;
    for (u32 r = 0; r < rounds; r++) {
        u8 needle = (u8)('a' + r % 4);
        for (u32 i = 0; i < n; i++)
            if (text_bytes[i] == needle && text_bytes[i + 1] != needle) found += i;
    }
    return found;
}

static u8 buffer[BIG(4096, 1000000) + 1];
static u32 numbers[BIG(512, 100000)];
static u64 cells[3][BIG(16 * 16, 96 * 96)];
static struct node nodes[BIG(1000, 100000)];

int main(void) {
    u64 mix = 0;
#define RUN(k, label, expression)                                                                          \
    if (WORKLOAD == 0 || WORKLOAD == k) {                                                                  \
        u64 value = (expression);                                                                          \
        line(label, value);                                                                                \
        mix = mix * 33 + value;                                                                            \
    }
    RUN(1, "fib", fib(BIG(18, 42)));
    RUN(2, "sieve", ({ u32 r = 0; for (u32 k = 0; k < BIG(2, 200); k++) r += sieve(buffer, BIG(4096, 1000000)); r; }));
    RUN(3, "hash", ({ for (u32 i = 0; i < 4096; i++) buffer[i] = (u8)(i * 7 + 3); hash(buffer, 4096, BIG(4, 100000)); }));
    RUN(4, "sort", sort_run(numbers, BIG(512, 100000), BIG(2, 60)));
    RUN(5, "interpret", interpret_run(BIG(20, 1000), BIG(50, 20000)));
    RUN(6, "matrix", matrix(cells[0], cells[1], cells[2], BIG(16, 96), BIG(2, 300)));
    RUN(7, "walk", walk(nodes, BIG(1000, 100000), BIG(3, 3000)));
    RUN(8, "search", search(buffer, BIG(4096, 1000000), BIG(2, 400)));
    return (int)(mix & 0x7f);
}

__attribute__((section(".text._start"), noreturn)) void _start(void) {
    int status = main();
    syscall3(SYS_exit_group, status, 0, 0);
    __builtin_unreachable();
}
