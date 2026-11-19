// Recursion DEPTH calls deep with a frame at every level, which needs Linux's 8 MiB stack.
#include <stdio.h>

static volatile long seen;

__attribute__((noinline)) static long down(long n) {
    if (n == 0) return 0;
    long below = down(n - 1);
    seen = below;
    return below + n % 7;
}

int main(void) {
    printf("%ld\n", down(DEPTH));
    return 0;
}
