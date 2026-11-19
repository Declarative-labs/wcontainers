// The virtual counter, read without a system call, never goes backwards, nor does CLOCK_MONOTONIC,
// also when the container is saved and brought back between reads.
#include <stdint.h>
#include <stdio.h>
#include <time.h>
#include <unistd.h>

static uint64_t counter(void) {
    uint64_t value;
    __asm__ volatile("isb; mrs %0, cntvct_el0" : "=r"(value));
    return value;
}

static uint64_t monotonic(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (uint64_t)ts.tv_sec * 1000000000ull + ts.tv_nsec;
}

int main(void) {
    uint64_t frequency;
    __asm__ volatile("mrs %0, cntfrq_el0" : "=r"(frequency));
    uint64_t last = counter(), last_monotonic = monotonic(), worst = 0;
    int backwards = 0, monotonic_backwards = 0;
    for (int i = 0; i < 4000; i++) {
        getppid();
        uint64_t now = counter(), now_monotonic = monotonic();
        if (now < last) {
            backwards++;
            if (last - now > worst) worst = last - now;
        }
        if (now_monotonic < last_monotonic) monotonic_backwards++;
        last = now;
        last_monotonic = now_monotonic;
    }
    printf("frequency nonzero %d\n", frequency != 0);
    printf("counter backwards %d\n", backwards);
    printf("monotonic backwards %d\n", monotonic_backwards);
    if (backwards) fprintf(stderr, "worst step back %llu ticks\n", (unsigned long long)worst);
    return 0;
}
