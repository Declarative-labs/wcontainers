// One function of many thousands of blocks, more than the translator puts in one piece.
#include <stdio.h>

static volatile unsigned long input = 0x5a5aa5a5c3c33c3cul;
// A store in each branch keeps the compiler from turning the branches into selects.
static volatile unsigned long hits;

#define S1(n) if ((x >> ((n) & 63)) & 1) { y = y * 3 + (n); hits = y; } else y ^= (unsigned long)(n) << ((n) & 7);
#define S4(n) S1(n) S1((n) + 1) S1((n) + 2) S1((n) + 3)
#define S16(n) S4(n) S4((n) + 4) S4((n) + 8) S4((n) + 12)
#define S64(n) S16(n) S16((n) + 16) S16((n) + 32) S16((n) + 48)
#define S256(n) S64(n) S64((n) + 64) S64((n) + 128) S64((n) + 192)
#define S1024(n) S256(n) S256((n) + 256) S256((n) + 512) S256((n) + 768)

static unsigned long many(unsigned long x) {
    unsigned long y = 1;
    S1024(0) S1024(1024) S1024(2048)
    return y;
}

int main(void) {
    unsigned long x = input, total = 0;
    for (int i = 0; i < 20; i++) {
        total += many(x);
        x = x * 6364136223846793005ul + 1442695040888963407ul;
    }
    printf("%lx %lx\n", total, hits);
    return 0;
}
