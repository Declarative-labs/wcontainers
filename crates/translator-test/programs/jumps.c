// setjmp and longjmp: jumps out of recursion of every depth, out of nested setjmp levels, and back into a
// loop, printing what each jump brought back.
#include <setjmp.h>
#include <stdio.h>

static jmp_buf outer, inner;
static volatile int reached;

__attribute__((noinline)) static void dive(int n, jmp_buf to, int value) {
    volatile char frame[48];
    frame[n % 48] = (char)n;
    reached = n + frame[n % 48] - (char)n;
    if (n == 0) longjmp(to, value);
    dive(n - 1, to, value);
    printf("never %d\n", n);
}

__attribute__((noinline)) static int nested(int n) {
    volatile int kept = n * 3;
    int got = setjmp(inner);
    if (got == 0) dive(n, inner, n + 1);
    if (n % 3 == 0) dive(n / 2, outer, 100 + n);
    return kept + got;
}

int main(void) {
    volatile int total = 0;
    for (volatile int round = 0; round < 40; round++) {
        int got = setjmp(outer);
        if (got == 0) {
            total += nested(round);
            dive(round, outer, -1 - round);
        }
        total += got * 7 + reached;
        printf("round %d: %d, %d\n", round, got, total);
    }
    return total & 0x7f;
}
