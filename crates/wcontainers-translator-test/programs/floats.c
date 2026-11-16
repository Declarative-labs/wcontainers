// A floating point workload: an n-body step in double precision and a single precision matrix product,
// for timing vector and floating point registers. SCALE=1 gives sizes for timing.

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
static float a[N][N], b[N][N], c[N][N];

int main(void) {
    for (int i = 0; i < BODIES; i++) {
        mass[i] = 1.0 + i * 0.01;
        for (int k = 0; k < 3; k++) position[i][k] = i * 1.5 + k, velocity[i][k] = 0.01 * (i - k);
    }
    int steps = SCALE ? 2000000 : 1000;
    for (int s = 0; s < steps; s++) advance(0.01);
    for (int i = 0; i < N; i++)
        for (int j = 0; j < N; j++) a[i][j] = (i * 7 + j) % 13 * 0.25f, b[i][j] = (i + j * 3) % 11 * 0.5f;
    int rounds = SCALE ? 400 : 2;
    for (int r = 0; r < rounds; r++)
        for (int i = 0; i < N; i++)
            for (int j = 0; j < N; j++) {
                float sum = c[i][j] * 0.5f;
                for (int k = 0; k < N; k++) sum += a[i][k] * b[k][j];
                c[i][j] = sum;
            }
    printf("%.12f %.12f %.6e\n", position[0][0], velocity[4][2], c[N - 1][N - 1]);
    return 0;
}
