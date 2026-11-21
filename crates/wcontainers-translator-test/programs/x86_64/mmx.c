// MMX through inline assembly (compilers now lower MMX intrinsics to SSE): every arithmetic, compare, logic,
// pack, unpack and shift instruction on register and memory operands, the SSE and SSSE3 additions on MMX
// registers, the conversions between MMX and SSE registers, `maskmovq` and `movntq`, and how MMX meets the
// x87 unit: the tags and TOP an MMX instruction leaves, `emms`, the register aliasing seen through `fxsave`,
// and an x87 load into a full stack left by MMX.

#include <stdint.h>
#include <stdio.h>
#include <string.h>

static const uint64_t inputs[] = {
    0, ~0ull, 0x8000000000000000ull, 0x7fff7fff7fff7fffull, 0x0123456789abcdefull,
    0xfedcba9876543210ull, 0x00ff00ff80807f7full, 0x8000800080008000ull, 0x0001000200030004ull,
    0x7f80017f80ff0102ull,
};
#define INPUTS (sizeof inputs / sizeof inputs[0])

static uint64_t hash(uint64_t h, uint64_t v) {
    for (int i = 0; i < 8; i++) h = (h ^ (v >> (8 * i) & 0xff)) * 0x100000001b3ull;
    return h;
}

// An instruction of two MMX operands, from a register and from memory.
#define BINARY(insn)                                                                                     \
    static uint64_t insn##_register(uint64_t a, uint64_t b) {                                            \
        uint64_t r;                                                                                      \
        __asm__("movq %1, %%mm0\n\tmovq %2, %%mm5\n\t" #insn " %%mm5, %%mm0\n\tmovq %%mm0, %0"           \
                : "=r"(r) : "r"(a), "r"(b) : "mm0", "mm5");                                               \
        return r;                                                                                        \
    }                                                                                                    \
    static uint64_t insn##_memory(uint64_t a, uint64_t b) {                                              \
        uint64_t r;                                                                                      \
        __asm__("movq %1, %%mm3\n\t" #insn " %2, %%mm3\n\tmovq %%mm3, %0" : "=r"(r) : "r"(a), "m"(b) : "mm3"); \
        return r;                                                                                        \
    }

#define EACH_BINARY(X)                                                                                   \
    X(paddb) X(paddw) X(paddd) X(paddq) X(paddsb) X(paddsw) X(paddusb) X(paddusw)                        \
    X(psubb) X(psubw) X(psubd) X(psubq) X(psubsb) X(psubsw) X(psubusb) X(psubusw)                        \
    X(pmullw) X(pmulhw) X(pmulhuw) X(pmuludq) X(pmaddwd)                                                 \
    X(pcmpeqb) X(pcmpeqw) X(pcmpeqd) X(pcmpgtb) X(pcmpgtw) X(pcmpgtd)                                    \
    X(pand) X(pandn) X(por) X(pxor)                                                                      \
    X(packsswb) X(packssdw) X(packuswb)                                                                  \
    X(punpcklbw) X(punpcklwd) X(punpckldq) X(punpckhbw) X(punpckhwd) X(punpckhdq)                        \
    X(psllw) X(pslld) X(psllq) X(psrlw) X(psrld) X(psrlq) X(psraw) X(psrad)                              \
    X(pavgb) X(pavgw) X(pmaxsw) X(pmaxub) X(pminsw) X(pminub) X(psadbw)                                  \
    X(pshufb) X(phaddw) X(phaddd) X(phaddsw) X(phsubw) X(phsubd) X(phsubsw)                              \
    X(pmaddubsw) X(pmulhrsw) X(psignb) X(psignw) X(psignd) X(pabsb) X(pabsw) X(pabsd)

EACH_BINARY(BINARY)

#define SHIFT_IMMEDIATE(name, insn, imm)                                                                 \
    static uint64_t name##_register(uint64_t a, uint64_t b) {                                            \
        (void)b;                                                                                         \
        uint64_t r;                                                                                      \
        __asm__("movq %1, %%mm7\n\t" #insn " $" #imm ", %%mm7\n\tmovq %%mm7, %0" : "=r"(r) : "r"(a) : "mm7"); \
        return r;                                                                                        \
    }
SHIFT_IMMEDIATE(psllw_3, psllw, 3)
SHIFT_IMMEDIATE(psllw_17, psllw, 17)
SHIFT_IMMEDIATE(pslld_9, pslld, 9)
SHIFT_IMMEDIATE(psllq_33, psllq, 33)
SHIFT_IMMEDIATE(psrlw_5, psrlw, 5)
SHIFT_IMMEDIATE(psrld_31, psrld, 31)
SHIFT_IMMEDIATE(psrlq_1, psrlq, 1)
SHIFT_IMMEDIATE(psrlq_64, psrlq, 64)
SHIFT_IMMEDIATE(psraw_7, psraw, 7)
SHIFT_IMMEDIATE(psraw_16, psraw, 16)
SHIFT_IMMEDIATE(psrad_20, psrad, 20)

#define SHUFFLE(name, imm)                                                                               \
    static uint64_t name##_register(uint64_t a, uint64_t b) {                                            \
        uint64_t r;                                                                                      \
        __asm__("movq %1, %%mm2\n\tpshufw $" #imm ", %%mm2, %%mm1\n\tpaddw %2, %%mm1\n\tmovq %%mm1, %0"  \
                : "=r"(r) : "r"(a), "m"(b) : "mm1", "mm2");                                               \
        return r;                                                                                        \
    }
SHUFFLE(pshufw_1b, 0x1b)
SHUFFLE(pshufw_b1, 0xb1)

#define ALIGN(name, imm)                                                                                 \
    static uint64_t name##_register(uint64_t a, uint64_t b) {                                            \
        uint64_t r;                                                                                      \
        __asm__("movq %1, %%mm4\n\tmovq %2, %%mm6\n\tpalignr $" #imm ", %%mm6, %%mm4\n\tmovq %%mm4, %0"  \
                : "=r"(r) : "r"(a), "r"(b) : "mm4", "mm6");                                               \
        return r;                                                                                        \
    }
ALIGN(palignr_3, 3)
ALIGN(palignr_9, 9)
ALIGN(palignr_17, 17)

// Words in and out of an MMX register, and its byte mask.
static uint64_t words(uint64_t a, uint64_t b) {
    uint64_t r;
    uint32_t low, mask;
    __asm__("movq %3, %%mm1\n\tpinsrw $2, %k4, %%mm1\n\tpextrw $1, %%mm1, %1\n\tpmovmskb %%mm1, %2\n\t"
            "movq %%mm1, %0"
            : "=&r"(r), "=&r"(low), "=&r"(mask) : "r"(a), "r"(b) : "mm1");
    return r ^ (uint64_t)low << 7 ^ (uint64_t)mask << 40;
}

// movd and movq between MMX, general registers and memory.
static uint64_t moves(uint64_t a, uint64_t b) {
    uint64_t r, m = b;
    uint32_t d;
    __asm__("movd %k2, %%mm2\n\tmovq %%mm2, %%mm6\n\tpsllq $32, %%mm6\n\tmovd %%mm2, %1\n\tmovq (%3), %%mm3\n\t"
            "por %%mm3, %%mm6\n\tmovq %%mm6, (%3)\n\tmovq %%mm6, %0"
            : "=&r"(r), "=&r"(d) : "r"(a), "r"(&m) : "mm2", "mm3", "mm6", "memory");
    return r ^ m << 1 ^ d;
}

// MMX to and from SSE registers, and the conversions of packed integers and floats.
static uint64_t to_sse(uint64_t a, uint64_t b) {
    uint64_t r, s;
    __asm__("movq %2, %%mm0\n\tmovq2dq %%mm0, %%xmm1\n\tmovq %3, %%xmm2\n\tpunpcklqdq %%xmm2, %%xmm1\n\t"
            "pshufd $0x4e, %%xmm1, %%xmm1\n\tmovdq2q %%xmm1, %%mm4\n\tmovq %%mm4, %0\n\tmovq %%xmm2, %1"
            : "=&r"(r), "=&r"(s) : "r"(a), "r"(b) : "mm0", "mm4", "xmm1", "xmm2");
    return r ^ s << 1;
}

static uint64_t conversions(uint64_t a, uint64_t b) {
    uint64_t singles, truncated, rounded, doubles, back;
    __asm__("movq %5, %%mm0\n\txorps %%xmm0, %%xmm0\n\tcvtpi2ps %%mm0, %%xmm0\n\tmovq %%xmm0, %0\n\t"
            "movq %6, %%xmm3\n\tcvttps2pi %%xmm3, %%mm1\n\tmovq %%mm1, %1\n\tcvtps2pi %%xmm3, %%mm2\n\t"
            "movq %%mm2, %2\n\tcvtpi2pd %%mm0, %%xmm4\n\tmovq %%xmm4, %3\n\tcvtpd2pi %%xmm4, %%mm5\n\t"
            "movq %%mm5, %4"
            : "=&r"(singles), "=&r"(truncated), "=&r"(rounded), "=&r"(doubles), "=&r"(back)
            : "r"(a), "r"(b)
            : "mm0", "mm1", "mm2", "mm5", "xmm0", "xmm3", "xmm4");
    return singles ^ truncated << 3 ^ rounded << 5 ^ doubles << 7 ^ back << 11;
}

// maskmovq writes the bytes whose mask bit is set at rdi; movntq writes all eight.
static uint64_t masked(uint64_t a, uint64_t b) {
    uint64_t out[2] = {0x5555555555555555ull, 0};
    __asm__ volatile("movq %1, %%mm0\n\tmovq %2, %%mm1\n\tmaskmovq %%mm1, %%mm0\n\tmovntq %%mm1, 8(%0)\n\tsfence"
                     : : "D"(out), "r"(a), "r"(b) : "mm0", "mm1", "memory");
    return out[0] ^ out[1] << 1;
}

typedef uint64_t (*operation)(uint64_t, uint64_t);
static const struct {
    const char *name;
    operation f;
} operations[] = {
#define LIST(insn) {#insn " register", insn##_register}, {#insn " memory", insn##_memory},
    EACH_BINARY(LIST)
#define LIST_ONE(name) {#name, name##_register},
    LIST_ONE(psllw_3) LIST_ONE(psllw_17) LIST_ONE(pslld_9) LIST_ONE(psllq_33) LIST_ONE(psrlw_5)
    LIST_ONE(psrld_31) LIST_ONE(psrlq_1) LIST_ONE(psrlq_64) LIST_ONE(psraw_7) LIST_ONE(psraw_16)
    LIST_ONE(psrad_20) LIST_ONE(pshufw_1b) LIST_ONE(pshufw_b1) LIST_ONE(palignr_3) LIST_ONE(palignr_9)
    LIST_ONE(palignr_17)
    {"words", words}, {"moves", moves}, {"to sse", to_sse}, {"conversions", conversions},
    {"masked", masked},
};

// The x87 state as fnstenv stores it: control, status and the full tag word.
static void environment(const char *when) {
    uint16_t env[14];
    __asm__ volatile("fnstenv %0\n\tfldenv %0" : "=m"(env));
    printf("%s: control %04x status %04x tags %04x\n", when, env[0], env[2], env[4]);
}

int main(void) {
    for (size_t o = 0; o < sizeof operations / sizeof operations[0]; o++) {
        uint64_t h = 0xcbf29ce484222325ull, first = operations[o].f(inputs[4], inputs[5]);
        for (size_t i = 0; i < INPUTS; i++)
            for (size_t j = 0; j < INPUTS; j++) h = hash(h, operations[o].f(inputs[i], inputs[j]));
        __asm__ volatile("emms");
        printf("%-18s %016llx %016llx\n", operations[o].name, (unsigned long long)first, (unsigned long long)h);
    }

    environment("after emms");
    uint64_t seen = 0x0123456789abcdefull;
    __asm__ volatile("movq %0, %%mm3" : : "r"(seen) : "mm3");
    environment("after an MMX move");

    // fxsave shows MMX register 3 as x87 register R3 with its exponent all ones.
    static uint8_t area[512] __attribute__((aligned(16)));
    __asm__ volatile("fxsave %0" : "=m"(area));
    uint64_t mantissa;
    uint16_t exponent;
    memcpy(&mantissa, area + 32 + 3 * 16, sizeof mantissa);
    memcpy(&exponent, area + 32 + 3 * 16 + 8, sizeof exponent);
    printf("fxsave: control %04x status %04x abridged tags %02x mm3 %016llx exponent %04x\n",
           *(uint16_t *)area, *(uint16_t *)(area + 2), area[4], (unsigned long long)mantissa, exponent);

    // An x87 load with every register in use overflows the stack: invalid, with the stack fault flag.
    long double loaded;
    uint16_t status;
    __asm__ volatile("fnclex\n\tfld1\n\tfnstsw %1\n\tfstpt %0" : "=m"(loaded), "=m"(status));
    printf("load over MMX: status %04x %Lg\n", status, loaded);
    __asm__ volatile("emms");
    environment("after the load and emms");

    // Two x87 pushes, then an MMX write: TOP goes back to 0 and every register is in use; emms empties them.
    uint16_t pushed[14], written[14], emptied[14];
    __asm__ volatile("fld1\n\tfld1\n\tfnstenv %0\n\tfldenv %0\n\tpxor %%mm0, %%mm0\n\tfnstenv %1\n\t"
                     "fldenv %1\n\temms\n\tfnstenv %2\n\tfldenv %2"
                     : "=m"(pushed), "=m"(written), "=m"(emptied) : : "mm0", "memory");
    printf("two pushed: status %04x tags %04x; after pxor: status %04x tags %04x; after emms: tags %04x\n",
           pushed[2], pushed[4], written[2], written[4], emptied[4]);
    long double third = 1.0L / (long double)(inputs[8] >> 48 | 2);
    printf("third %La\n", third);
    return 0;
}
