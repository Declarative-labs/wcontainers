// Calls of tiny leaf functions in a loop, where a progress count kept in memory would pay at every call and
// return. WORKLOAD 1 calls a leaf placed below its caller, 2 one placed above it; 0 runs both. SCALE 0 is
// small enough for look-ahead none, SCALE 1 is for timing. Prints each loop's result.
#include <stdio.h>

#ifndef WORKLOAD
#define WORKLOAD 0
#endif
#ifndef SCALE
#define SCALE 0
#endif
#define CALLS (SCALE ? 300000000UL : 2000UL)

typedef unsigned long u64;

__attribute__((noinline)) u64 leaf_below(u64 x, u64 i) { return x * 31 + i; }

__attribute__((noinline)) u64 run_below(u64 n) {
    u64 x = 1;
    for (u64 i = 0; i < n; i++) x = leaf_below(x, i);
    return x;
}

u64 leaf_above(u64 x, u64 i);

__attribute__((noinline)) u64 run_above(u64 n) {
    u64 x = 1;
    for (u64 i = 0; i < n; i++) x = leaf_above(x, i);
    return x;
}

__attribute__((noinline)) u64 leaf_above(u64 x, u64 i) { return x * 37 + i; }

int main(void) {
    if (WORKLOAD == 0 || WORKLOAD == 1) printf("%lx\n", run_below(CALLS));
    if (WORKLOAD == 0 || WORKLOAD == 2) printf("%lx\n", run_above(CALLS));
    return 0;
}
