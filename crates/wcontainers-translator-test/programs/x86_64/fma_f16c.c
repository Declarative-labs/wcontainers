// FMA's fused multiply-adds, F16C's half precision conversions and AVX's floating point on ymm registers, in
// MXCSR's default mode and in others, with the exception flags read back. Built for a program told the
// x86-64-v3 list.
#include <immintrin.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#define V3 __attribute__((target("avx2,fma,f16c")))

enum {
    VALUES = 64,
    ROUNDS = 4,
    FLAGS = 0x3f,
    DEFAULT_MXCSR = 0x1f80,
    ROUNDING = 0x6000,
    ROUNDING_SHIFT = 13,
    DAZ = 0x40,
    FTZ = 0x8000,
};

static float singles[VALUES] __attribute__((aligned(32)));
static double doubles[VALUES] __attribute__((aligned(32)));
static double stored[VALUES];

// Values a float unit's edges are among: zeros, denormals, the largest, infinities, not-a-numbers, and
// numbers whose product needs more than the format's bits.
static const uint32_t edges[] = {
    0x00000000, 0x80000000, 0x00000001, 0x807fffff, 0x7f7fffff, 0x7f800000, 0xff800000, 0x7fc00001,
    0x7fa00002, 0x3f800001, 0xbf7ffffe, 0x33800000, 0x38800000, 0x477fe000, 0x47800000, 0x3eaaaaab,
};

static uint64_t state = 0x2545f4914f6cdd1dULL;
static uint32_t next(void) {
    state = state * 6364136223846793005ULL + 1442695040888963407ULL;
    return (uint32_t)(state >> 32);
}

static void show_bits(const char *what, const void *v, int words) {
    uint32_t w[8];
    memcpy(w, v, (size_t)words * sizeof w[0]);
    printf("%s:", what);
    for (int i = 0; i < words; i++) {
        printf(" %08x", w[i]);
    }
    printf("\n");
}

V3 static void show(const char *what, __m256 v) { show_bits(what, &v, 8); }
V3 static void show_pd(const char *what, __m256d v) { show_bits(what, &v, 8); }
V3 static void show128(const char *what, __m128 v) { show_bits(what, &v, 4); }

V3 static __m256 ps(int at) { return _mm256_loadu_ps(&singles[at % (VALUES - 8)]); }
V3 static __m256d pd(int at) { return _mm256_loadu_pd(&doubles[at % (VALUES - 4)]); }

V3 static void fused(int round) {
    __m256 a = ps(round), b = ps(round * 3 + 1), c = ps(round * 5 + 2);
    show("fmadd_ps", _mm256_fmadd_ps(a, b, c));
    show("fmsub_ps", _mm256_fmsub_ps(a, b, c));
    show("fnmadd_ps", _mm256_fnmadd_ps(a, b, c));
    show("fnmsub_ps", _mm256_fnmsub_ps(a, b, c));
    show("fmaddsub_ps", _mm256_fmaddsub_ps(a, b, c));
    show("fmsubadd_ps", _mm256_fmsubadd_ps(a, b, c));
    __m256d x = pd(round), y = pd(round * 3 + 1), z = pd(round * 7 + 2);
    show_pd("fmadd_pd", _mm256_fmadd_pd(x, y, z));
    show_pd("fmsubadd_pd", _mm256_fmsubadd_pd(x, y, z));
    show128("fmadd_ss", _mm_fmadd_ss(_mm256_castps256_ps128(a), _mm256_castps256_ps128(b),
                                     _mm256_castps256_ps128(c)));
    __m128d s = _mm_fnmsub_sd(_mm256_castpd256_pd128(x), _mm256_castpd256_pd128(y),
                              _mm256_castpd256_pd128(z));
    show_bits("fnmsub_sd", &s, 4);
    // A dot product the compiler writes with its own choice of forms.
    double sum = 0;
    for (int i = 0; i < VALUES; i++) {
        sum = fma(doubles[i], doubles[(i * 7 + round) % VALUES], sum);
    }
    show_bits("dot", &sum, 2);
}

V3 static void halves(int round) {
    __m256 a = ps(round * 2);
    __m128i nearest = _mm256_cvtps_ph(a, _MM_FROUND_TO_NEAREST_INT);
    __m128i down = _mm256_cvtps_ph(a, _MM_FROUND_TO_NEG_INF);
    __m128i up = _mm256_cvtps_ph(a, _MM_FROUND_TO_POS_INF);
    __m128i zero = _mm256_cvtps_ph(a, _MM_FROUND_TO_ZERO);
    __m128i current = _mm256_cvtps_ph(a, _MM_FROUND_CUR_DIRECTION);
    show_bits("ph nearest", &nearest, 4);
    show_bits("ph down", &down, 4);
    show_bits("ph up", &up, 4);
    show_bits("ph zero", &zero, 4);
    show_bits("ph current", &current, 4);
    show("ph back", _mm256_cvtph_ps(nearest));
    show128("ph back 128", _mm_cvtph_ps(_mm_cvtps_ph(_mm256_castps256_ps128(a), 0)));
    uint16_t memory[8];
    _mm_storeu_si128((__m128i *)memory, _mm256_cvtps_ph(a, _MM_FROUND_TO_NEAREST_INT));
    show_bits("ph stored", memory, 4);
}

V3 static void wide_floats(int round) {
    __m256 a = ps(round + 4), b = ps(round * 9 + 5);
    show("hadd_ps", _mm256_hadd_ps(a, b));
    show("dp_ps", _mm256_dp_ps(a, b, 0xf1));
    show("sqrt_ps", _mm256_sqrt_ps(a));
    show("div_ps", _mm256_div_ps(a, b));
    show("max_ps", _mm256_max_ps(a, b));
    show("cmp_ps", _mm256_cmp_ps(a, b, _CMP_NGE_UQ));
    show("permutevar_ps", _mm256_permutevar_ps(a, _mm256_castps_si256(b)));
    show("blendv_ps", _mm256_blendv_ps(a, b, ps(round + 6)));
    show_pd("round_pd", _mm256_round_pd(pd(round), _MM_FROUND_TO_NEG_INF | _MM_FROUND_NO_EXC));
    show_pd("broadcast_sd", _mm256_broadcast_sd(&doubles[round]));
    show_pd("cvtps_pd", _mm256_cvtps_pd(_mm256_castps256_ps128(a)));
    show128("cvtpd_ps", _mm256_cvtpd_ps(pd(round + 1)));
    __m256i index = _mm256_and_si256(_mm256_castps_si256(b), _mm256_set1_epi32(VALUES - 1));
    show("i32gather_ps", _mm256_i32gather_ps(singles, index, 4));
    show_pd("mask_i32gather_pd", _mm256_mask_i32gather_pd(pd(round), doubles,
                                                          _mm256_castsi256_si128(index),
                                                          pd(round + 2), 8));
    __m256i mask = _mm256_castps_si256(ps(round + 3));
    show_pd("maskload_pd", _mm256_maskload_pd(&doubles[round], mask));
    memset(stored, 0, sizeof stored);
    _mm256_maskstore_pd(&stored[round], mask, pd(round + 5));
    _mm_maskstore_ps((float *)&stored[round + 4], _mm256_castsi256_si128(mask),
                     _mm256_castps256_ps128(b));
    show_bits("maskstore", &stored[round], 16);
}

// The same work in another rounding mode, with denormals as zero and flush to zero, its flags read back.
V3 static void in_mode(unsigned mxcsr) {
    _mm_setcsr(mxcsr);
    printf("mxcsr %04x\n", mxcsr);
    fused(1);
    halves(1);
    wide_floats(1);
    unsigned flags = _mm_getcsr() & FLAGS;
    _mm_setcsr(DEFAULT_MXCSR);
    printf("flags %02x\n", flags);
}

int main(void) {
    // The flags are cleared first, as feclearexcept does, so that the program's later reads see them all.
    _mm_setcsr(DEFAULT_MXCSR);
    for (int i = 0; i < VALUES; i++) {
        uint32_t bits = i % 3 == 0 ? edges[next() % (sizeof edges / sizeof edges[0])] : next();
        memcpy(&singles[i], &bits, sizeof bits);
        uint64_t wide = (uint64_t)next() << 32 | next();
        if (i % 5 == 0) {
            wide = (uint64_t)bits << 32;
        }
        memcpy(&doubles[i], &wide, sizeof wide);
    }
    for (int round = 0; round < ROUNDS; round++) {
        printf("round %d\n", round);
        fused(round);
        halves(round);
        wide_floats(round);
    }
    printf("flags %02x\n", _mm_getcsr() & FLAGS);
    for (unsigned rounding = 1; rounding <= ROUNDING >> ROUNDING_SHIFT; rounding++) {
        in_mode(DEFAULT_MXCSR | rounding << ROUNDING_SHIFT);
    }
    in_mode(DEFAULT_MXCSR | DAZ | FTZ);
    return 0;
}
