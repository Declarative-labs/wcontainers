// Floating point under rounding modes other than the default, so every operation takes the exact path
// through the float hook: products, sums, quotients, square roots and fused multiply-adds, with the
// exception flags read at the end of each mode.

#include <fenv.h>
#include <math.h>
#include <stdio.h>

#ifndef STEPS
#define STEPS 20000
#endif

static double run(int mode, double x) {
    fesetround(mode);
    feclearexcept(FE_ALL_EXCEPT);
    volatile double v = x, sum = 0;
    for (int i = 1; i <= STEPS; i++) {
        v = v * 1.0000001 + 1.0 / (double)i;
        sum += sqrt(v) / 3.0;
        sum = fma(v, 1e-9, sum);
    }
    printf("mode %d sum %a value %a flags %x\n", mode, (double)sum, (double)v, fetestexcept(FE_ALL_EXCEPT));
    return sum;
}

int main(void) {
    double total = run(FE_UPWARD, 0.1) + run(FE_TOWARDZERO, 0.3) + run(FE_DOWNWARD, 0.7);
    fesetround(FE_TONEAREST);
    printf("total %a\n", total);
    return 0;
}
