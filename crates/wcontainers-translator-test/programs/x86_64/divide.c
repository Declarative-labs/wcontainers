// x86-64's `div` of a 128-bit dividend whose high half is not zero, which no compiler's 64-bit division
// gives, ROUNDS times: the cost of the translated long division against the CPU's own. HIGH=0 divides with
// a zero high half instead, the translation's fast path. Prints a sum of the results and exits 0.

typedef unsigned long u64;

#include "syscall.h"

#ifndef ROUNDS
#define ROUNDS 50000000
#endif
#ifndef HIGH
#define HIGH 1
#endif

void _start(void) {
    u64 sum = 0, low = 0x0123456789abcdef;
    // Above every high half, so every quotient fits.
    const u64 divisor = 0x9e3779b97f4a7c15;
    for (u64 i = 0; i < ROUNDS; i++) {
        u64 high = HIGH ? (i & 0xffffff) + 1 : 0, quotient, remainder;
        __asm__ volatile("divq %4"
                         : "=a"(quotient), "=d"(remainder)
                         : "a"(low), "d"(high), "r"(divisor));
        sum += quotient ^ remainder;
        low = low * 6364136223846793005ul + 1442695040888963407ul;
    }
    char text[17];
    for (int n = 15; n >= 0; n--, sum >>= 4) text[n] = "0123456789abcdef"[sum & 15];
    text[16] = '\n';
    syscall3(SYS_write, 1, (long)text, 17);
    syscall3(SYS_exit, 0, 0, 0);
    __builtin_unreachable();
}
