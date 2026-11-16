// Floating point and vector workloads: an n-body step in double precision, single precision matrix products
// (one the compiler vectorises) and sums of absolute differences of bytes, for timing vector and floating
// point registers. SCALE=1 gives sizes for timing.

#include <math.h>
#include <stdio.h>

#ifndef SCALE
#define SCALE 0
#endif

#define BODIES 5

static double position[BODIES][3], velocity[BODIES][3], mass[BODIES];

static void advance(double dt) {
    for (int i = 0; i < BODIES; i++)
        for (int j = i + 1; j < BODIES; j++) {
            double d[3], d2 = 0;
            for (int k = 0; k < 3; k++) d[k] = position[i][k] - position[j][k], d2 += d[k] * d[k];
            double magnitude = dt / (d2 * sqrt(d2));
            for (int k = 0; k < 3; k++) {
                velocity[i][k] -= d[k] * mass[j] * magnitude;
                velocity[j][k] += d[k] * mass[i] * magnitude;
            }
        }
    for (int i = 0; i < BODIES; i++)
        for (int k = 0; k < 3; k++) position[i][k] += dt * velocity[i][k];
}

#define N 64
static float a[N][N], b[N][N], c[N][N], d[N][N];

// Bytes for the integer vector workload.
#define BYTES 4096
static unsigned char x[BYTES], y[BYTES];

int main(void) {
    // WORKLOAD picks one part for timing: 1 the n-body step (scalar double), 2 the matrix product with the sum
    // innermost (scalar single), 3 the product in i, k, j order (single, vectorised), 4 sums of absolute
    // differences of bytes (integer vectors). 0, the default, runs them all.
#ifndef WORKLOAD
#define WORKLOAD 0
#endif
#define RUNS(n) (WORKLOAD == 0 || WORKLOAD == (n))
    for (int i = 0; i < BODIES; i++) {
        mass[i] = 1.0 + i * 0.01;
        for (int k = 0; k < 3; k++) position[i][k] = i * 1.5 + k, velocity[i][k] = 0.01 * (i - k);
    }
    int steps = SCALE ? 10000000 : 1000;
    if (RUNS(1))
        for (int s = 0; s < steps; s++) advance(0.01);
    for (int i = 0; i < N; i++)
        for (int j = 0; j < N; j++) a[i][j] = (i * 7 + j) % 13 * 0.25f, b[i][j] = (i + j * 3) % 11 * 0.5f;
    int rounds = SCALE ? 2000 : 2;
    if (RUNS(2))
        for (int r = 0; r < rounds; r++)
            for (int i = 0; i < N; i++)
                for (int j = 0; j < N; j++) {
                    float sum = c[i][j] * 0.5f;
                    for (int k = 0; k < N; k++) sum += a[i][k] * b[k][j];
                    c[i][j] = sum;
                }
    if (RUNS(3))
        for (int r = 0; r < 10 * rounds; r++)
            for (int i = 0; i < N; i++) {
                for (int j = 0; j < N; j++) d[i][j] *= 0.5f;
                for (int k = 0; k < N; k++)
                    for (int j = 0; j < N; j++) d[i][j] += a[i][k] * b[k][j];
            }
    for (int i = 0; i < BYTES; i++) x[i] = i * 37 + (i >> 5), y[i] = i * 11 ^ (i >> 3);
    unsigned total = 0;
    int passes = SCALE ? 400000 : 10;
    if (RUNS(4))
        for (int p = 0; p < passes; p++) {
            unsigned short sum = 0;
            for (int i = 0; i < BYTES; i++) sum += x[i] > y[i] ? x[i] - y[i] : y[i] - x[i];
            total += sum;
            x[p % BYTES] ^= sum;
        }
    printf("%.12f %.12f %.6e %.6e %u\n", position[0][0], velocity[4][2], c[N - 1][N - 1], d[N - 1][N - 1], total);
    return 0;
}
