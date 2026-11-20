// Jumps through a register to code of their own function that exploring it does not reach: a switch on
// k & 15, whose table the compiler bounds by no check, and a threaded interpreter's computed goto. Prints
// how many of each it made.
#include <stdio.h>
#ifndef ROUNDS
#define ROUNDS 1200
#endif

static volatile unsigned long sink;
static unsigned long switches, gotos;

#define CASE(n, e) \
    case n:        \
        y = (e);   \
        sink = y;  \
        break;

__attribute__((noinline)) unsigned long pick(unsigned long x, unsigned k) {
    unsigned long y = x;
    switches++;
    switch (k & 15) {
        CASE(0, y * 3 + 1)
        CASE(1, y ^ 0x55)
        CASE(2, y + 77)
        CASE(3, y * 5 - 9)
        CASE(4, (y << 3) ^ y)
        CASE(5, y >> 1 | 1)
        CASE(6, y * 11 + 2)
        CASE(7, y - 1234)
        CASE(8, ~y)
        CASE(9, y * 13 ^ 7)
        CASE(10, y + (y >> 7))
        CASE(11, y * 17 + 5)
        CASE(12, (y >> 3) * 9)
        CASE(13, y ^ (y << 11))
        CASE(14, y * 19 - 1)
        CASE(15, y + 0xabcdef)
    }
    return y;
}

enum { INC, DBL, DEC, MIX, LOOP, HALT };
static const unsigned char program[] = {INC, DBL, MIX, DEC, INC, LOOP, HALT};

__attribute__((noinline)) long interpret(const unsigned char *code, long n) {
    static void *const ops[] = {&&inc, &&dbl, &&dec, &&mix, &&loop, &&halt};
    const unsigned char *pc = code;
    long acc = 1, count = n;
#define NEXT          \
    do {              \
        gotos++;      \
        goto *ops[*pc++]; \
    } while (0)
    NEXT;
inc:
    acc += 3;
    NEXT;
dbl:
    acc = (acc * 2) ^ count;
    NEXT;
dec:
    count--;
    NEXT;
mix:
    acc = acc * 31 + (acc >> 5);
    NEXT;
loop:
    if (count > 0) pc = code;
    NEXT;
halt:
    return acc;
}

int main(void) {
    unsigned long x = 1, t = 0;
    for (unsigned k = 0; k < ROUNDS; k++) {
        t += pick(x, k * 7);
        x = x * 6364136223846793005ul + 1442695040888963407ul;
    }
    long acc = interpret(program, ROUNDS);
    printf("%lx %lx %lx\n%lu %lu\n", t, sink, (unsigned long)acc, switches, gotos);
    return 0;
}
