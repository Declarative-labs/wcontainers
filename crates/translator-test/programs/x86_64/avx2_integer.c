// AVX2's integer, permute, gather and masked move instructions and BMI2, on drawn data: each result's bytes
// are printed. Built for a program told the x86-64-v3 list.
#include <immintrin.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#define V3 __attribute__((target("avx2,bmi2")))

enum {
    WORDS = 256,
    ROUNDS = 6,
    LANE_BITS = 32,
};

static uint32_t data[WORDS] __attribute__((aligned(32)));
static int64_t wide[WORDS];
static uint32_t stored[WORDS];

// A fixed linear congruential sequence (Knuth's MMIX constants).
static uint64_t state = 0x853c49e6748fea9bULL;
static uint32_t next(void) {
    state = state * 6364136223846793005ULL + 1442695040888963407ULL;
    return (uint32_t)(state >> LANE_BITS);
}

V3 static void show(const char *what, __m256i v) {
    uint32_t words[8];
    memcpy(words, &v, sizeof words);
    printf("%s:", what);
    for (int i = 0; i < 8; i++) {
        printf(" %08x", words[i]);
    }
    printf("\n");
}

V3 static __m256i load(int at) {
    return _mm256_loadu_si256((const __m256i *)&data[at % (WORDS - 8)]);
}

V3 static void arithmetic(int round) {
    __m256i a = load(round), b = load(round * 7 + 3);
    show("add", _mm256_add_epi32(a, b));
    show("mullo", _mm256_mullo_epi32(a, b));
    show("mul_epu32", _mm256_mul_epu32(a, b));
    show("sub64", _mm256_sub_epi64(a, b));
    show("min_epi32", _mm256_min_epi32(a, b));
    show("max_epu8", _mm256_max_epu8(a, b));
    show("adds_epi16", _mm256_adds_epi16(a, b));
    show("avg_epu8", _mm256_avg_epu8(a, b));
    show("sad", _mm256_sad_epu8(a, b));
    show("madd", _mm256_madd_epi16(a, b));
    show("maddubs", _mm256_maddubs_epi16(a, b));
    show("hadd", _mm256_hadd_epi32(a, b));
    show("abs", _mm256_abs_epi8(a));
    show("sign", _mm256_sign_epi16(a, b));
    show("cmpgt", _mm256_cmpgt_epi64(a, b));
    show("packs", _mm256_packs_epi32(a, b));
    show("packus", _mm256_packus_epi16(a, b));
    show("mpsadbw", _mm256_mpsadbw_epu8(a, b, 0x2d));
}

V3 static void permutes(int round) {
    __m256i a = load(round * 3), b = load(round * 5 + 1);
    __m256i index = _mm256_and_si256(load(round + 11), _mm256_set1_epi32(7));
    show("shuffle_epi8", _mm256_shuffle_epi8(a, b));
    show("permutevar8x32", _mm256_permutevar8x32_epi32(a, index));
    show("permute4x64", _mm256_permute4x64_epi64(a, 0x9c));
    show("permute2x128", _mm256_permute2x128_si256(a, b, 0x21));
    show("blend_epi32", _mm256_blend_epi32(a, b, 0xa6));
    show("blendv_epi8", _mm256_blendv_epi8(a, b, load(round + 17)));
    show("alignr", _mm256_alignr_epi8(a, b, 5));
    show("unpacklo", _mm256_unpacklo_epi8(a, b));
    show("unpackhi", _mm256_unpackhi_epi64(a, b));
    show("broadcastd", _mm256_broadcastd_epi32(_mm256_castsi256_si128(a)));
    show("broadcastq", _mm256_broadcastq_epi64(_mm256_castsi256_si128(b)));
    show("inserti128", _mm256_inserti128_si256(a, _mm256_castsi256_si128(b), 1));
    __m128i high = _mm256_extracti128_si256(a, 1);
    show("cvtepu8_epi32", _mm256_cvtepu8_epi32(high));
    show("cvtepi16_epi32", _mm256_cvtepi16_epi32(high));
    show("sllv", _mm256_sllv_epi32(a, _mm256_and_si256(b, _mm256_set1_epi32(0x3f))));
    show("srlv64", _mm256_srlv_epi64(a, _mm256_and_si256(b, _mm256_set1_epi64x(0x7f))));
    show("srav", _mm256_srav_epi32(a, _mm256_and_si256(b, _mm256_set1_epi32(0x3f))));
    show("slli_si256", _mm256_bslli_epi128(a, 3));
    printf("movemask %08x testz %d testc %d\n", (uint32_t)_mm256_movemask_epi8(a),
           _mm256_testz_si256(a, b), _mm256_testc_si256(a, b));
}

V3 static void gathers(int round) {
    __m256i index = _mm256_and_si256(load(round + 5), _mm256_set1_epi32(WORDS - 1));
    show("i32gather_epi32", _mm256_i32gather_epi32((const int *)data, index, 4));
    __m256i mask = load(round + 9);
    show("mask_i32gather_epi32",
         _mm256_mask_i32gather_epi32(load(round), (const int *)data, index, mask, 4));
    __m256i quads = _mm256_and_si256(load(round + 13), _mm256_set1_epi64x(WORDS - 1));
    show("i64gather_epi64", _mm256_i64gather_epi64((const long long *)wide, quads, 8));
    show("mask_i64gather_epi32",
         _mm256_castsi128_si256(_mm256_mask_i64gather_epi32(
             _mm_set1_epi32(-1), (const int *)data, quads, _mm256_castsi256_si128(mask), 4)));
    show("i32gather_epi64",
         _mm256_i32gather_epi64((const long long *)wide, _mm256_castsi256_si128(index), 8));
}

V3 static void masked(int round) {
    __m256i mask = load(round + 21);
    show("maskload_epi32", _mm256_maskload_epi32((const int *)&data[round], mask));
    show("maskload_epi64", _mm256_maskload_epi64((const long long *)&wide[round], mask));
    memset(stored, 0, sizeof stored);
    _mm256_maskstore_epi32((int *)&stored[round], mask, load(round + 2));
    _mm_maskstore_epi32((int *)&stored[round + 8], _mm256_castsi256_si128(load(round + 3)),
                        _mm256_castsi256_si128(load(round + 4)));
    show("maskstore", _mm256_loadu_si256((const __m256i *)&stored[round]));
    show("maskstore128", _mm256_loadu_si256((const __m256i *)&stored[round + 8]));
}

V3 static void bits(int round) {
    uint64_t x = (uint64_t)next() << LANE_BITS | next(), m = (uint64_t)next() << LANE_BITS | next();
    unsigned long long high;
    uint64_t low = _mulx_u64(x, m, &high);
    printf("pext %016llx pdep %016llx bzhi %08x mulx %016llx:%016llx rorx %016llx sarx %016llx\n",
           (unsigned long long)_pext_u64(x, m), (unsigned long long)_pdep_u64(x, m),
           _bzhi_u32((uint32_t)x, round * 7 % 40), high, (unsigned long long)low,
           (unsigned long long)((x >> 13) | (x << 51)), (unsigned long long)((int64_t)x >> (m & 63)));
}

int main(void) {
    for (int i = 0; i < WORDS; i++) {
        data[i] = next();
        wide[i] = (int64_t)((uint64_t)next() << LANE_BITS | next());
    }
    for (int round = 0; round < ROUNDS; round++) {
        printf("round %d\n", round);
        arithmetic(round);
        permutes(round);
        gathers(round);
        masked(round);
        bits(round);
    }
    return 0;
}
