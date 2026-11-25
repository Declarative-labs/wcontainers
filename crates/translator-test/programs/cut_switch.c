// A switch whose cases hold more blocks than one piece may, so the function is cut in parts and most cases
// lie in a part other than the switch's. Each case is taken twice.
#include <stdio.h>

static volatile unsigned long input = 0x5a5aa5a5c3c33c3cul;
// A store in each branch keeps the compiler from turning the branches into selects.
static volatile unsigned long hits;

#define S1(n) if ((x >> ((n) & 63)) & 1) { y = y * 3 + (n); hits = y; } else y ^= (unsigned long)(n) << ((n) & 7);
#define S4(n) S1(n) S1((n) + 1) S1((n) + 2) S1((n) + 3)
#define S16(n) S4(n) S4((n) + 4) S4((n) + 8) S4((n) + 12)
#define S64(n) S16(n) S16((n) + 16) S16((n) + 32) S16((n) + 48)
#define CASE(k) case k: S64((k) * 64) break;
#define CASES(k) CASE(k) CASE((k) + 1) CASE((k) + 2) CASE((k) + 3) CASE((k) + 4) CASE((k) + 5) CASE((k) + 6) CASE((k) + 7)
#define CASE_COUNT 48

// Not static, so the compiler cannot know the cases cover every k and checks k before the table.
__attribute__((noinline)) unsigned long cut(unsigned long x, unsigned k) {
    unsigned long y = 1;
    switch (k) {
        CASES(0) CASES(8) CASES(16) CASES(24) CASES(32) CASES(40)
    }
    return y;
}

int main(void) {
    unsigned long x = input, total = 0;
    for (unsigned k = 0; k < 2 * CASE_COUNT; k++) {
        total += cut(x, k * 7 % CASE_COUNT);
        x = x * 6364136223846793005ul + 1442695040888963407ul;
    }
    printf("%lx %lx\n", total, hits);
    return 0;
}
