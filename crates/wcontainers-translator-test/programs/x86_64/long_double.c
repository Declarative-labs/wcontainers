// C's long double, which x86-64 keeps in the x87 unit: sums, products and quotients at 64 bits of
// precision, square roots, conversions to and from integers and doubles, comparisons with infinities and
// not-a-numbers, every rounding mode, the exception flags, and printing.

#include <fenv.h>
#include <float.h>
#include <math.h>
#include <stdio.h>

static volatile long double zero = 0.0L;

static void show(const char *what, long double x) { printf("%s %La %.21Lg\n", what, x, x); }

int main(void) {
    long double sum = 0, product = 1, quotient = 1;
    for (int i = 1; i <= 60; i++) {
        sum += 1.0L / i;
        product *= 1.0L + 1.0L / ((long double)i * i);
        quotient = quotient / 3 + i;
    }
    show("sum", sum);
    show("product", product);
    show("quotient", quotient);

    long double root = sqrtl(2.0L);
    show("root", root);
    show("root squared less two", root * root - 2);
    show("tiny", LDBL_MIN / 4096);
    show("large", LDBL_MAX / 3 * 2);
    show("epsilon", LDBL_EPSILON);

    const int modes[] = {FE_TONEAREST, FE_DOWNWARD, FE_UPWARD, FE_TOWARDZERO};
    for (int m = 0; m < 4; m++) {
        fesetround(modes[m]);
        volatile long double third = 1.0L / 3, half = 2.5L;
        printf("mode %d: %La %La %La %lld %.17g\n", m, third, rintl(half), rintl(-half), llrintl(-7.5L),
               (double)third);
    }
    fesetround(FE_TONEAREST);

    feclearexcept(FE_ALL_EXCEPT);
    volatile long double exact = 0.5L + 0.25L;
    printf("inexact after an exact sum: %d\n", fetestexcept(FE_INEXACT) != 0);
    volatile long double inexact = 1.0L / 7;
    printf("inexact after 1/7: %d\n", fetestexcept(FE_INEXACT) != 0);
    volatile long double infinite = 1.0L / zero;
    printf("divide by zero: %d, overflow %d\n", fetestexcept(FE_DIVBYZERO) != 0, fetestexcept(FE_OVERFLOW) != 0);
    volatile long double nan = zero / zero;
    printf("invalid: %d\n", fetestexcept(FE_INVALID) != 0);
    printf("%Lg %Lg %Lg %d %d %d\n", exact, infinite, -infinite, isnan(nan), nan == nan, infinite > LDBL_MAX);

    long long whole = (long long)(sum * 1e15L);
    unsigned long long big = (unsigned long long)(product * 1e18L);
    int narrow = (int)-quotient;
    printf("%lld %llu %d %Lg %Lg\n", whole, big, narrow, (long double)whole, (long double)big);
    double d = (double)sum;
    float f = (float)product;
    printf("%.17g %.9g %La %La\n", d, f, (long double)d, (long double)f);

    long double values[] = {sum, -product, quotient, root, 0.0L, -0.0L, infinite, nan};
    int counts[3] = {0, 0, 0};
    for (int i = 0; i < 8; i++)
        for (int j = 0; j < 8; j++) {
            counts[0] += values[i] < values[j];
            counts[1] += values[i] == values[j];
            counts[2] += isgreaterequal(values[i], values[j]);
        }
    printf("%d %d %d %Lg %Lg\n", counts[0], counts[1], counts[2], fabsl(-product), copysignl(3, -zero));
    return 0;
}
