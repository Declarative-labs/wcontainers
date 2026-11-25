// Where ARM64 and WebAssembly differ: division, saturating conversion, fused multiply-add, not-a-number
// bits, rounding modes and status flags, flush to zero, both styles of atomics, and the system instructions
// a program may use. Every result is printed as bits, so the output matches Linux only if each is exact.
// WITHOUT_FENV leaves out the parts that use <fenv.h>, so the program never reads FPSR.

#include <fenv.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static uint64_t bits(double d) {
    uint64_t u;
    memcpy(&u, &d, 8);
    return u;
}

static uint32_t bits32(float f) {
    uint32_t u;
    memcpy(&u, &f, 4);
    return u;
}

static double from(uint64_t u) {
    double d;
    memcpy(&d, &u, 8);
    return d;
}

static void division(void) {
    int64_t values[] = {7, -7, 0, INT64_MIN, -1, 1, INT64_MAX};
    for (int i = 0; i < 7; i++)
        for (int j = 0; j < 7; j++) {
            int64_t q, a = values[i], b = values[j];
            uint64_t u;
            int32_t w;
            __asm__ volatile("sdiv %0, %1, %2" : "=r"(q) : "r"(a), "r"(b));
            __asm__ volatile("udiv %0, %1, %2" : "=r"(u) : "r"(a), "r"(b));
            __asm__ volatile("sdiv %w0, %w1, %w2" : "=r"(w) : "r"(a), "r"(b));
            printf("div %lld %lld: %lld %llu %d\n", (long long)a, (long long)b, (long long)q,
                   (unsigned long long)u, w);
        }
}

static void conversions(void) {
    double values[] = {NAN, -NAN, INFINITY, -INFINITY, 1e300, -1e300, 2.5, -2.5, 3.5, -0.5,
                       9.2233720368547758e18, 4294967295.5, -2147483648.5, 0.49999999999999994};
    for (unsigned i = 0; i < sizeof values / sizeof *values; i++) {
        double d = values[i];
        int64_t zs, ns, as, ms, ps;
        uint64_t zu;
        int32_t ws;
        __asm__("fcvtzs %0, %d1" : "=r"(zs) : "w"(d));
        __asm__("fcvtns %0, %d1" : "=r"(ns) : "w"(d));
        __asm__("fcvtas %0, %d1" : "=r"(as) : "w"(d));
        __asm__("fcvtms %0, %d1" : "=r"(ms) : "w"(d));
        __asm__("fcvtps %0, %d1" : "=r"(ps) : "w"(d));
        __asm__("fcvtzu %0, %d1" : "=r"(zu) : "w"(d));
        __asm__("fcvtzs %w0, %d1" : "=r"(ws) : "w"(d));
        printf("convert %016llx: %lld %lld %lld %lld %lld %llu %d\n", (unsigned long long)bits(d),
               (long long)zs, (long long)ns, (long long)as, (long long)ms, (long long)ps,
               (unsigned long long)zu, ws);
    }
}

static void fused(void) {
    // (1 + 2^-30)(1 - 2^-30) - 1 is -2^-60 fused and 0 rounded twice.
    volatile double a = 1.0 + 0x1p-30, b = 1.0 - 0x1p-30, c = -1.0;
    double r;
    __asm__("fmadd %d0, %d1, %d2, %d3" : "=w"(r) : "w"(a), "w"(b), "w"(c));
    printf("fmadd %016llx fma %016llx\n", (unsigned long long)bits(r),
           (unsigned long long)bits(fma(a, b, c)));
    volatile float x = 1.0f + 0x1p-13f, y = 1.0f - 0x1p-13f, z = -1.0f;
    printf("fmaf %08x\n", bits32(fmaf(x, y, z)));
}

static void nans(void) {
    uint64_t operands[] = {0x7ff0000000000001, 0xfff8000000000002, 0x7ff8000000000003, 0x3ff0000000000000,
                           0x7ff0000000000000, 0xfff0000000000000, 0};
    for (int i = 0; i < 7; i++)
        for (int j = 0; j < 7; j++) {
            double a = from(operands[i]), b = from(operands[j]), s, m, d, x, n, f;
            __asm__("fadd %d0, %d1, %d2" : "=w"(s) : "w"(a), "w"(b));
            __asm__("fmul %d0, %d1, %d2" : "=w"(m) : "w"(a), "w"(b));
            __asm__("fdiv %d0, %d1, %d2" : "=w"(d) : "w"(a), "w"(b));
            __asm__("fmax %d0, %d1, %d2" : "=w"(x) : "w"(a), "w"(b));
            __asm__("fminnm %d0, %d1, %d2" : "=w"(n) : "w"(a), "w"(b));
            __asm__("fmadd %d0, %d1, %d2, %d3" : "=w"(f) : "w"(a), "w"(b), "w"(from(operands[(i + j) % 7])));
            printf("nan %d %d: %016llx %016llx %016llx %016llx %016llx %016llx\n", i, j,
                   (unsigned long long)bits(s), (unsigned long long)bits(m), (unsigned long long)bits(d),
                   (unsigned long long)bits(x), (unsigned long long)bits(n), (unsigned long long)bits(f));
        }
    float single;
    __asm__("fcvt %s0, %d1" : "=w"(single) : "w"(from(0x7ff0000000000123)));
    printf("fcvt nan %08x\n", bits32(single));
}

#ifndef WITHOUT_FENV
static const char *names[] = {"nearest", "upward", "downward", "towardzero"};
static const int modes[] = {FE_TONEAREST, FE_UPWARD, FE_DOWNWARD, FE_TOWARDZERO};

static void rounding(void) {
    volatile double third = 1.0, three = 3.0, big = 1e308, tiny = 1e-308;
    for (int i = 0; i < 4; i++) {
        fesetround(modes[i]);
        double q = third / three, p = big * 10.0, t = tiny / 1e10, s = sqrt(2.0 + third);
        float f = (float)(third / three);
        long long r = llrint(2.5 + third);
        printf("%s: %016llx %016llx %016llx %016llx %08x %lld %.17g\n", names[i],
               (unsigned long long)bits(q), (unsigned long long)bits(p), (unsigned long long)bits(t),
               (unsigned long long)bits(s), bits32(f), r, q * three);
    }
    fesetround(FE_TONEAREST);
}

static void flags(void) {
    volatile double one = 1.0, zero = 0.0, big = 1e308, tiny = 1e-308;
    feclearexcept(FE_ALL_EXCEPT);
    volatile double r = one / 3.0;
    printf("inexact %d", fetestexcept(FE_ALL_EXCEPT));
    feclearexcept(FE_ALL_EXCEPT);
    r = one / zero;
    printf(" divide %d", fetestexcept(FE_ALL_EXCEPT));
    feclearexcept(FE_ALL_EXCEPT);
    r = big * big;
    printf(" overflow %d", fetestexcept(FE_ALL_EXCEPT));
    feclearexcept(FE_ALL_EXCEPT);
    r = tiny * tiny;
    printf(" underflow %d", fetestexcept(FE_ALL_EXCEPT));
    feclearexcept(FE_ALL_EXCEPT);
    r = sqrt(-one);
    printf(" invalid %d", fetestexcept(FE_ALL_EXCEPT));
    feclearexcept(FE_ALL_EXCEPT);
    r = one + one;
    printf(" exact %d\n", fetestexcept(FE_ALL_EXCEPT));
    (void)r;
}
#endif

static void flush(void) {
    uint64_t fpcr;
    __asm__ volatile("mrs %0, fpcr" : "=r"(fpcr));
    __asm__ volatile("msr fpcr, %0" ::"r"(fpcr | (1 << 24)));
    volatile double denormal = 1e-310, small = 1e-300;
    double s = denormal + 0.0, p = small * 1e-10;
    __asm__ volatile("msr fpcr, %0" ::"r"(fpcr));
    printf("flush %016llx %016llx\n", (unsigned long long)bits(s), (unsigned long long)bits(p));
}

static void atomics(void) {
    uint64_t word = 40, old, expected = 42, pair[2] __attribute__((aligned(16))) = {1, 2};
    uint32_t small = 7;
    __asm__ volatile(".arch_extension lse\n\tldadd %1, %0, [%2]" : "=r"(old) : "r"((uint64_t)2), "r"(&word) : "memory");
    printf("ldadd %llu %llu\n", (unsigned long long)old, (unsigned long long)word);
    __asm__ volatile(".arch_extension lse\n\tcas %0, %1, [%2]" : "+r"(expected) : "r"((uint64_t)99), "r"(&word) : "memory");
    printf("cas %llu %llu\n", (unsigned long long)expected, (unsigned long long)word);
    __asm__ volatile(".arch_extension lse\n\tswpal %w1, %w0, [%2]" : "=r"(old) : "r"(3), "r"(&small) : "memory");
    printf("swp %llu %u\n", (unsigned long long)old, small);
    __asm__ volatile(".arch_extension lse\n\tldsmax %w1, %w0, [%2]" : "=r"(old) : "r"(-5), "r"(&small) : "memory");
    printf("ldsmax %llu %u\n", (unsigned long long)old, small);
    register uint64_t x0 __asm__("x0") = 1, x1 __asm__("x1") = 2, x2 __asm__("x2") = 10,
                                 x3 __asm__("x3") = 20;
    __asm__ volatile(".arch_extension lse\n\tcasp %0, %1, %2, %3, [%4]"
                     : "+r"(x0), "+r"(x1)
                     : "r"(x2), "r"(x3), "r"(pair)
                     : "memory");
    printf("casp %llu %llu %llu %llu\n", (unsigned long long)x0, (unsigned long long)x1,
           (unsigned long long)pair[0], (unsigned long long)pair[1]);
    // The older style: a load and store exclusive loop.
    uint32_t status;
    do {
        __asm__ volatile("ldaxr %0, [%2]\n\tadd %0, %0, #5\n\tstlxr %w1, %0, [%2]"
                         : "=&r"(old), "=&r"(status)
                         : "r"(&word)
                         : "memory");
    } while (status);
    printf("exclusive %llu\n", (unsigned long long)word);
    // A store exclusive with nothing remembered fails.
    __asm__ volatile("clrex\n\tstxr %w0, %1, [%2]" : "=&r"(status) : "r"((uint64_t)1), "r"(&word) : "memory");
    printf("failed %u %llu\n", status, (unsigned long long)word);
    // What the compiler makes of C's atomics.
    long counter = 0;
    for (int i = 0; i < 10; i++) __atomic_fetch_add(&counter, i, __ATOMIC_SEQ_CST);
    long want = 45;
    int swapped = __atomic_compare_exchange_n(&counter, &want, 7, 0, __ATOMIC_SEQ_CST, __ATOMIC_SEQ_CST);
    printf("c11 %d %ld\n", swapped, (long)counter);
}

static void system_instructions(void) {
    uint64_t a, b, frequency, zva, tp;
    __asm__ volatile("mrs %0, cntvct_el0" : "=r"(a));
    __asm__ volatile("mrs %0, cntvct_el0" : "=r"(b));
    __asm__ volatile("mrs %0, cntfrq_el0" : "=r"(frequency));
    __asm__ volatile("mrs %0, dczid_el0" : "=r"(zva));
    __asm__ volatile("mrs %0, tpidr_el0" : "=r"(tp));
    printf("timer %d %d dczid %llu thread pointer %d\n", b >= a, frequency > 0, (unsigned long long)zva,
           tp != 0);
    static unsigned char block[256] __attribute__((aligned(64)));
    memset(block, 0xa5, sizeof block);
    __asm__ volatile("dc zva, %0" ::"r"(block + 64 + 13) : "memory");
    __asm__ volatile("dc cvau, %0\n\tdsb ish\n\tic ivau, %0\n\tdsb ish\n\tisb" ::"r"(block) : "memory");
    int zeros = 0;
    for (int i = 0; i < 256; i++) zeros += block[i] == 0;
    printf("dc zva %d %d %d\n", zeros, block[63], block[128]);
    __asm__ volatile("hint #25\n\thint #29\n\thint #34\n\tyield\n\tdmb ish\n\tdsb sy\n\tisb\n\tclrex");
    printf("hints\n");
}

static int compare(const void *a, const void *b) {
    double x = *(const double *)a, y = *(const double *)b;
    return (x > y) - (x < y);
}

static void printing(void) {
    double values[] = {0.1, 1.0 / 3.0, 1e300, 5e-324, -0.0, 123456789.125, 2.5, 1e21, 0x1.fffffffffffffp1023};
    for (unsigned i = 0; i < sizeof values / sizeof *values; i++)
        printf("%.17g %a %e %f %g\n", values[i], values[i], values[i], values[i] < 1e30 ? values[i] : 0, values[i]);
    printf("%lld %llx %d %u %hhd %#o\n", -9223372036854775807LL - 1, 0xdeadbeefcafeULL, -42, 4000000000u, (char)200, 0755);
    double sorted[64];
    unsigned seed = 12345;
    for (int i = 0; i < 64; i++) {
        seed = seed * 1103515245 + 12345;
        sorted[i] = (double)(int)(seed >> 8) / 1000.0;
    }
    qsort(sorted, 64, sizeof *sorted, compare);
    double sum = 0;
    for (int i = 0; i < 64; i++) sum = sum * 1.0001 + sorted[i];
    printf("sorted %g %g %.17g\n", sorted[0], sorted[63], sum);
    printf("strtod %a %a\n", strtod("0x1.8p3", 0), strtod("2.2250738585072011e-308", 0));
}

static void vectors(void) {
    float a[67], b[67];
    double c[33];
    int8_t d[64];
    for (int i = 0; i < 67; i++) a[i] = i * 0.25f - 3, b[i] = 1.0f / (i + 1);
    for (int i = 0; i < 33; i++) c[i] = i * 1.5 - 7;
    for (int i = 0; i < 64; i++) d[i] = (int8_t)(i * 37);
    float dot = 0;
    for (int i = 0; i < 67; i++) dot += a[i] * b[i];
    double product = 1;
    for (int i = 0; i < 33; i++) product *= c[i] + 0.5;
    int saturated = 0, maximum = -128;
    for (int i = 0; i < 64; i++) {
        int v = d[i] * 3;
        saturated += v > 127 ? 127 : v < -128 ? -128 : v;
        maximum = d[i] > maximum ? d[i] : maximum;
    }
    printf("vectors %08x %016llx %d %d\n", bits32(dot), (unsigned long long)bits(product), saturated, maximum);
}

int main(void) {
    division();
    conversions();
    fused();
    nans();
#ifndef WITHOUT_FENV
    rounding();
    flags();
#endif
    flush();
    atomics();
    system_instructions();
    printing();
    vectors();
    return 0;
}
