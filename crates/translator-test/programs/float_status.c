// FPSR's cumulative flags read without clearing them first: after a division by zero, an overflow, an
// underflow and an invalid operation, through fetestexcept and through mrs. Exits with the flags at the end.
#include <fenv.h>
#include <math.h>
#include <stdio.h>

static unsigned long fpsr(void) {
    unsigned long value;
    __asm__ volatile("mrs %0, fpsr" : "=r"(value));
    return value;
}

int main(void) {
    volatile double one = 1.0, zero = 0.0, big = 1e308, tiny = 1e-308, minus = -1.0;
    printf("at the start: fpsr %lx, %x\n", fpsr(), fetestexcept(FE_ALL_EXCEPT));
    volatile double r = one / zero;
    printf("after 1/0: fpsr %lx, divbyzero %d\n", fpsr(), fetestexcept(FE_DIVBYZERO) != 0);
    r = big * big;
    printf("after overflow: %x\n", fetestexcept(FE_ALL_EXCEPT));
    r = tiny * tiny;
    printf("after underflow: %x\n", fetestexcept(FE_ALL_EXCEPT));
    r = sqrt(minus);
    printf("after sqrt(-1): %lx %d\n", fpsr(), r != r);
    return fetestexcept(FE_ALL_EXCEPT) & 0x7f;
}
