// FMA's fused multiply-adds and F16C's half precision conversions in MXCSR's default mode, which the program
// never reads or writes: zeros, denormals, infinities, not-a-numbers of both kinds and products that need more
// than the format's bits, packed and scalar, in every order. Built for a program told the x86-64-v3 list.
#include <immintrin.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#define V3 __attribute__((target("avx2,fma,f16c")))

enum {
    VALUES = 96,
    ROUNDS = 12,
};

static float singles[VALUES] __attribute__((aligned(32)));
static double doubles[VALUES] __attribute__((aligned(32)));

static const uint32_t single_edges[] = {
    0x00000000, 0x80000000, 0x00000001, 0x807fffff, 0x7f7fffff, 0x7f800000, 0xff800000, 0x7fc00001,
    0xffa00002, 0x3f800001, 0xbf7ffffe, 0x33800000, 0x387fc000, 0x477fefff, 0x47800000, 0x3eaaaaab,
    0x7fbfffff, 0xffc00000, 0x00800000, 0xb3000001,
};
static const uint64_t double_edges[] = {
    0x0000000000000000, 0x8000000000000000, 0x0000000000000001, 0x800fffffffffffff,
    0x7fefffffffffffff, 0x7ff0000000000000, 0xfff0000000000000, 0x7ff8000000000003,
    0xfff4000000000005, 0x3ff0000000000001, 0xbfeffffffffffffe, 0x3ca0000000000000,
    0x7ff7ffffffffffff, 0x3fd5555555555555,
};

static uint64_t state = 0x9e3779b97f4a7c15ULL;
static uint32_t next(void) {
    state = state * 6364136223846793005ULL + 1442695040888963407ULL;
    return (uint32_t)(state >> 32);
}

static void show(const char *what, const void *v, int words) {
    uint32_t w[8];
    memcpy(w, v, (size_t)words * sizeof w[0]);
    printf("%s:", what);
    for (int i = 0; i < words; i++) {
        printf(" %08x", w[i]);
    }
    printf("\n");
}

V3 static void fused(int round) {
    __m256 a = _mm256_loadu_ps(&singles[round % (VALUES - 8)]);
    __m256 b = _mm256_loadu_ps(&singles[(round * 7 + 3) % (VALUES - 8)]);
    __m256 c = _mm256_loadu_ps(&singles[(round * 11 + 5) % (VALUES - 8)]);
    __m256d x = _mm256_loadu_pd(&doubles[round % (VALUES - 4)]);
    __m256d y = _mm256_loadu_pd(&doubles[(round * 5 + 1) % (VALUES - 4)]);
    __m256d z = _mm256_loadu_pd(&doubles[(round * 13 + 2) % (VALUES - 4)]);
    __m256 r;
    __m256d d;
    __m128 s;
    __m128d t;
    r = _mm256_fmadd_ps(a, b, c), show("fmadd_ps", &r, 8);
    r = _mm256_fmsub_ps(a, b, c), show("fmsub_ps", &r, 8);
    r = _mm256_fnmadd_ps(a, b, c), show("fnmadd_ps", &r, 8);
    r = _mm256_fnmsub_ps(a, b, c), show("fnmsub_ps", &r, 8);
    r = _mm256_fmaddsub_ps(a, b, c), show("fmaddsub_ps", &r, 8);
    r = _mm256_fmsubadd_ps(c, a, b), show("fmsubadd_ps", &r, 8);
    d = _mm256_fmadd_pd(x, y, z), show("fmadd_pd", &d, 8);
    d = _mm256_fnmsub_pd(z, x, y), show("fnmsub_pd", &d, 8);
    d = _mm256_fmaddsub_pd(y, z, x), show("fmaddsub_pd", &d, 8);
    s = _mm_fmadd_ps(_mm256_castps256_ps128(a), _mm256_castps256_ps128(b), _mm256_castps256_ps128(c));
    show("fmadd_ps 128", &s, 4);
    s = _mm_fmsub_ss(_mm256_castps256_ps128(b), _mm256_castps256_ps128(c), _mm256_castps256_ps128(a));
    show("fmsub_ss", &s, 4);
    t = _mm_fnmadd_sd(_mm256_castpd256_pd128(x), _mm256_castpd256_pd128(z), _mm256_castpd256_pd128(y));
    show("fnmadd_sd", &t, 4);
    t = _mm_fmadd_pd(_mm256_castpd256_pd128(z), _mm256_castpd256_pd128(y), _mm256_castpd256_pd128(x));
    show("fmadd_pd 128", &t, 4);
}

V3 static void half(int round) {
    __m256 a = _mm256_loadu_ps(&singles[(round * 3) % (VALUES - 8)]);
    __m128i h;
    __m256 back;
    h = _mm256_cvtps_ph(a, _MM_FROUND_TO_NEAREST_INT), show("cvtps_ph nearest", &h, 4);
    back = _mm256_cvtph_ps(h), show("cvtph_ps", &back, 8);
    h = _mm256_cvtps_ph(a, _MM_FROUND_TO_NEG_INF), show("cvtps_ph down", &h, 4);
    h = _mm256_cvtps_ph(a, _MM_FROUND_TO_POS_INF), show("cvtps_ph up", &h, 4);
    h = _mm256_cvtps_ph(a, _MM_FROUND_TO_ZERO), show("cvtps_ph toward zero", &h, 4);
    h = _mm256_cvtps_ph(a, _MM_FROUND_CUR_DIRECTION), show("cvtps_ph as MXCSR says", &h, 4);
    h = _mm_cvtps_ph(_mm256_castps256_ps128(a), _MM_FROUND_TO_NEAREST_INT), show("cvtps_ph 128", &h, 2);
    __m128 narrow = _mm_cvtph_ps(h);
    show("cvtph_ps 128", &narrow, 4);
}

int main(void) {
    int single_count = sizeof single_edges / sizeof single_edges[0];
    int double_count = sizeof double_edges / sizeof double_edges[0];
    for (int i = 0; i < VALUES; i++) {
        uint32_t s = i < single_count ? single_edges[i] : next();
        uint64_t d = i < double_count ? double_edges[i] : (uint64_t)next() << 32 | next();
        // Half the random values near one, so that products and sums are close and round.
        if (i >= single_count && i % 2) {
            s = (s & 0x807fffff) | 0x3f000000;
        }
        if (i >= double_count && i % 2) {
            d = (d & 0x800fffffffffffffULL) | 0x3fe0000000000000ULL;
        }
        memcpy(&singles[i], &s, sizeof s);
        memcpy(&doubles[i], &d, sizeof d);
    }
    for (int round = 0; round < ROUNDS; round++) {
        printf("round %d\n", round);
        fused(round);
        half(round);
    }
    return 0;
}
