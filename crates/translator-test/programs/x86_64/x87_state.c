// The x87 unit's own instructions beyond C's arithmetic: precision control, the environment and state
// instructions (fnstenv, fldenv, fnsave, frstor, fxsave, fxrstor), fxam's classes, compares into the flags
// and the condition codes, fcmov, the integer and packed decimal loads and stores, fisttp, frndint, fscale,
// fxtract, fprem and fprem1 with their partial remainders, the constants in every rounding mode, the stack's
// own instructions, and the exception flags with every exception masked, the stack fault among them.

#include <stdint.h>
#include <stdio.h>
#include <string.h>

typedef struct {
    uint64_t mantissa;
    uint16_t exponent;
} __attribute__((packed)) extended;

static long double from_bits(uint16_t exponent, uint64_t mantissa) {
    extended e = {mantissa, exponent};
    long double x;
    memcpy(&x, &e, sizeof e);
    return x;
}

static void show(const char *what, long double x) {
    extended e;
    memcpy(&e, &x, sizeof e);
    printf("%-22s %04x %016llx %.21Lg\n", what, e.exponent, (unsigned long long)e.mantissa, x);
}

static uint16_t status(void) {
    uint16_t s;
    __asm__ volatile("fnstsw %0" : "=m"(s) : : "memory");
    return s;
}

static void control(uint16_t c) { __asm__ volatile("fldcw %0" : : "m"(c) : "memory"); }

static volatile long double values[] = {1.5L, -2.75L, 1e300L, 3.0L, 0.1L, 1e-4000L, 7.0L, -0.0L};
#define VALUES (sizeof values / sizeof values[0])

// A sum and a quotient at each precision control and rounding control.
static void precisions(void) {
    for (int precision = 0; precision < 4; precision++)
        for (int rounding = 0; rounding < 4; rounding++) {
            control((uint16_t)(0x37f & ~0xf00) | precision << 8 | rounding << 10);
            volatile long double third = 1.0L / values[3], sum = values[0] + values[4];
            volatile long double tenth = values[4] / values[6];
            uint16_t flags = status();
            control(0x37f);
            printf("precision %d rounding %d: %La %La %La flags %04x\n", precision, rounding, third, sum, tenth,
                   flags);
            __asm__ volatile("fnclex");
        }
}

// fxam's condition codes for each class, the empty register among them.
static void classes(void) {
    const long double kinds[] = {0.0L, -0.0L, 1.0L, -1e-4940L, 1.0L / 0.0L, __builtin_nanl(""),
                                 from_bits(0x7fff, 0x4000000000000000ull), from_bits(0x0000, 0x8000000000000000ull),
                                 from_bits(0x4000, 0x4000000000000000ull)};
    for (size_t i = 0; i < sizeof kinds / sizeof kinds[0]; i++) {
        uint16_t s;
        __asm__ volatile("fldt %1\n\tfxam\n\tfnstsw %0\n\tfstp %%st(0)" : "=m"(s) : "m"(kinds[i]));
        printf("fxam %zu: %04x\n", i, s & 0x4700);
    }
    uint16_t s;
    __asm__ volatile("ffree %%st(0)\n\tfxam\n\tfnstsw %0" : "=m"(s));
    printf("fxam empty: %04x\n", s & 0x4700);
}

// fcomi, fucomi, fcom, fucom, ftst and fcmov between every pair of values.
static void compares(void) {
    const long double kinds[] = {1.0L, -2.0L, 1.0L / 0.0L, __builtin_nanl(""), -0.0L, 0.0L};
    const size_t n = sizeof kinds / sizeof kinds[0];
    for (size_t i = 0; i < n; i++)
        for (size_t j = 0; j < n; j++) {
            uint64_t flags, unordered;
            uint16_t s, t;
            long double picked;
            __asm__ volatile("fnclex\n\tfldt %5\n\tfldt %6\n\tfcomi %%st(1), %%st\n\tpushfq\n\tpopq %0\n\t"
                             "fucomi %%st(1), %%st\n\tpushfq\n\tpopq %1\n\tfcom %%st(1)\n\tfnstsw %2\n\t"
                             "fcmovb %%st(1), %%st\n\tftst\n\tfnstsw %3\n\tfstpt %4\n\tfstp %%st(0)"
                             : "=&r"(flags), "=&r"(unordered), "=m"(s), "=m"(t), "=m"(picked)
                             : "m"(kinds[i]), "m"(kinds[j]));
            printf("compare %zu %zu: %03llx %03llx %04x %04x %La\n", i, j, (unsigned long long)(flags & 0x8d5),
                   (unsigned long long)(unordered & 0x8d5), s & 0x4781, t & 0x4781, picked);
        }
}

// Integer stores of each width in each rounding mode, fisttp, and packed decimal both ways.
static void integers(void) {
    const long double kinds[] = {2.5L, -2.5L, 3.5L, 40000.0L, -32768.0L, 3e9L, -1e19L, 123456789012345678.0L};
    for (size_t i = 0; i < sizeof kinds / sizeof kinds[0]; i++)
        for (int rounding = 0; rounding < 4; rounding++) {
            int16_t w;
            int32_t d;
            int64_t q, t;
            control((uint16_t)(0x37f | rounding << 10));
            __asm__ volatile("fnclex\n\tfldt %4\n\tfists %0\n\tfistl %1\n\tfistpll %2\n\tfldt %4\n\tfisttpll %3"
                             : "=m"(w), "=m"(d), "=m"(q), "=m"(t) : "m"(kinds[i]));
            uint16_t flags = status();
            control(0x37f);
            printf("integer %zu %d: %d %d %lld %lld flags %04x\n", i, rounding, w, d, (long long)q, (long long)t,
                   flags & 0x3f);
        }
    for (size_t i = 0; i < sizeof kinds / sizeof kinds[0]; i++) {
        uint8_t bcd[10];
        long double back;
        __asm__ volatile("fnclex\n\tfldt %2\n\tfbstp %0\n\tfbld %0\n\tfstpt %1" : "=m"(bcd), "=m"(back)
                         : "m"(kinds[i]));
        printf("decimal %zu:", i);
        for (int b = 9; b >= 0; b--) printf(" %02x", bcd[b]);
        printf(" back %Lg flags %04x\n", back, status() & 0x3f);
    }
}

// frndint in each mode, fscale, fxtract, and fprem and fprem1 to the end through their partial remainders.
static void rounding(void) {
    for (size_t i = 0; i < VALUES; i++)
        for (int mode = 0; mode < 4; mode++) {
            long double r, scaled, significand, exponent;
            control((uint16_t)(0x37f | mode << 10));
            __asm__ volatile("fnclex\n\tfldt %4\n\tfrndint\n\tfstpt %0\n\tfldt %5\n\tfldt %4\n\tfscale\n\t"
                             "fstpt %1\n\tfstp %%st(0)\n\tfldt %4\n\tfxtract\n\tfstpt %2\n\tfstpt %3"
                             : "=m"(r), "=m"(scaled), "=m"(significand), "=m"(exponent)
                             : "m"(values[i]), "m"(values[(i + 1) % VALUES]));
            uint16_t flags = status();
            control(0x37f);
            printf("round %zu %d: %La %La %La %La flags %04x\n", i, mode, r, scaled, significand, exponent,
                   flags & 0x3f);
        }
    const long double dividends[] = {1e300L, -7.0L, 1e4000L, 5.5L};
    const long double divisors[] = {3.0L, 2.0L, 0.7L, -1.5L};
    for (size_t i = 0; i < 4; i++)
        for (int nearest = 0; nearest < 2; nearest++) {
            long double r = dividends[i];
            int passes = 0;
            uint16_t s;
            do {
                if (nearest)
                    __asm__("fldt %2\n\tfldt %1\n\tfprem1\n\tfnstsw %0\n\tfstpt %1\n\tfstp %%st(0)"
                            : "=m"(s), "+m"(r) : "m"(divisors[i]));
                else
                    __asm__("fldt %2\n\tfldt %1\n\tfprem\n\tfnstsw %0\n\tfstpt %1\n\tfstp %%st(0)"
                            : "=m"(s), "+m"(r) : "m"(divisors[i]));
                passes++;
            } while (s & 0x400);
            printf("remainder %zu %d: %La after %d passes, quotient bits %d%d%d\n", i, nearest, r, passes,
                   !!(s & 0x200), !!(s & 0x4000), !!(s & 0x100));
        }
}

// The constants in each rounding mode.
static void constants(void) {
    for (int mode = 0; mode < 4; mode++) {
        long double c[7];
        control((uint16_t)(0x37f | mode << 10));
        __asm__ volatile("fldpi\n\tfstpt %0\n\tfldl2e\n\tfstpt %1\n\tfldl2t\n\tfstpt %2\n\tfldlg2\n\tfstpt %3\n\t"
                         "fldln2\n\tfstpt %4\n\tfld1\n\tfstpt %5\n\tfldz\n\tfstpt %6"
                         : "=m"(c[0]), "=m"(c[1]), "=m"(c[2]), "=m"(c[3]), "=m"(c[4]), "=m"(c[5]), "=m"(c[6]));
        control(0x37f);
        printf("constants %d:", mode);
        for (int i = 0; i < 7; i++) printf(" %La", c[i]);
        printf("\n");
    }
}

// The exception flags with everything masked, the stack's overflow and underflow among them.
// (The assembler's `fdivp` with no operands is DE F1: ST(1) = ST(0) / ST(1), then a pop.)
static void exceptions(void) {
    long double out;
    uint16_t s[8];
    const long double big = 1e4000L, small = 1e-4000L, two = 2.0L, tiny = from_bits(0, 0x1234);
    __asm__ volatile("fninit\n\tfldt %9\n\tfmul %%st(0), %%st\n\tfnstsw %0\n\tfstp %%st(0)\n\t"
                     "fnclex\n\tfldt %10\n\tfmul %%st(0), %%st\n\tfnstsw %1\n\tfstp %%st(0)\n\t"
                     "fnclex\n\tfldz\n\tfldt %11\n\tfdivp\n\tfnstsw %2\n\tfstp %%st(0)\n\t"
                     "fnclex\n\tfldz\n\tfldz\n\tfdivp\n\tfnstsw %3\n\tfstpt %8\n\t"
                     "fnclex\n\tfadd %%st(0), %%st\n\tfnstsw %4\n\tffree %%st(0)\n\t"
                     "fnclex\n\tfldt %12\n\tfldt %11\n\tfmulp\n\tfnstsw %5\n\tfstp %%st(0)\n\t"
                     "fnclex\n\tfldt %11\n\tfsqrt\n\tfchs\n\tfsqrt\n\tfnstsw %6\n\tfstp %%st(0)\n\t"
                     "fnclex\n\tfld1\n\tfld1\n\tfld1\n\tfld1\n\tfld1\n\tfld1\n\tfld1\n\tfld1\n\tfld1\n\tfnstsw %7\n\t"
                     "fninit"
                     : "=m"(s[0]), "=m"(s[1]), "=m"(s[2]), "=m"(s[3]), "=m"(s[4]), "=m"(s[5]), "=m"(s[6]),
                       "=m"(s[7]), "=m"(out)
                     : "m"(big), "m"(small), "m"(two), "m"(tiny));
    for (int i = 0; i < 8; i++) printf("exception %d: %04x\n", i, s[i]);
    show("zero over zero", out);
}

// The environment and the whole state, stored, changed and loaded back.
static void states(void) {
    uint16_t env[14], saved[54], after[54];
    static uint8_t area[512] __attribute__((aligned(16))), again[512] __attribute__((aligned(16)));
    long double a, b, c;
    control(0x0e7f);
    __asm__ volatile("fldpi\n\tfld1\n\tfldt %3\n\tfnstenv %0\n\tfldenv %0\n\tfnsave %1\n\tfrstor %1\n\t"
                     "fxsave %2\n\tfninit"
                     : "=m"(env), "=m"(saved), "=m"(area) : "m"(values[2]));
    printf("environment: control %04x status %04x tags %04x\n", env[0], env[2], env[4]);
    printf("saved: control %04x status %04x tags %04x\n", saved[0], saved[2], saved[4]);
    for (int r = 0; r < 3; r++) {
        long double x;
        memcpy(&x, (uint8_t *)saved + 28 + 10 * r, sizeof(extended));
        show("saved register", x);
    }
    printf("fxsave: control %04x status %04x abridged tags %02x mxcsr %08x\n", *(uint16_t *)area,
           *(uint16_t *)(area + 2), area[4], *(uint32_t *)(area + 24));
    // Change the stored status's TOP and a register, then load it back.
    *(uint16_t *)(area + 2) = (uint16_t)((*(uint16_t *)(area + 2) & ~0x3800) | 6 << 11);
    area[4] = 0xc3;
    __asm__ volatile("fxrstor %4\n\tfstpt %0\n\tfstpt %1\n\tfstpt %2\n\tfxsave %3\n\tfninit"
                     : "=m"(a), "=m"(b), "=m"(c), "=m"(again) : "m"(area));
    show("after fxrstor 0", a);
    show("after fxrstor 1", b);
    show("after fxrstor 2", c);
    printf("again: control %04x status %04x abridged tags %02x\n", *(uint16_t *)again, *(uint16_t *)(again + 2),
           again[4]);
    __asm__ volatile("fldz\n\tfldl2e\n\tfnsave %0\n\tfrstor %0\n\tfstp %%st(0)\n\tfstp %%st(0)\n\tfnsave %1"
                     : "+m"(saved), "=m"(after));
    printf("after fnsave: control %04x status %04x tags %04x\n", after[0], after[2], after[4]);
    control(0x37f);
}

// The stack's own instructions: fxch, fincstp, fdecstp, ffree, and pops of what they leave.
static void stack(void) {
    long double x, y, z;
    uint16_t s, t;
    __asm__ volatile("fldpi\n\tfld1\n\tfldz\n\tfxch %%st(2)\n\tfincstp\n\tfnstsw %3\n\tfdecstp\n\tffree %%st(1)\n\t"
                     "fstpt %0\n\tfstpt %1\n\tfnstsw %4\n\tfstpt %2\n\tfninit"
                     : "=m"(x), "=m"(y), "=m"(z), "=m"(s), "=m"(t));
    printf("stack: %04x %04x\n", s, t);
    show("first", x);
    show("second", y);
    show("third", z);
}

int main(void) {
    precisions();
    classes();
    compares();
    integers();
    rounding();
    constants();
    exceptions();
    states();
    stack();
    return 0;
}
